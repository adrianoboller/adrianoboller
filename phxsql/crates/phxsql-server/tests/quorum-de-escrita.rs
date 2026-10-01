//! A escrita com quorum -- pedido 207 -- provada PELO SOQUETE, com tres nos
//! de verdade: o master neste processo e as duas replicas como `phxsqld`
//! filhos, mortos com `SIGKILL` quando a prova pede uma replica caida.
//!
//! O que depende do sistema operacional (a conexao que a replica abre e
//! mantem, o processo que some no meio de uma espera) se prova contra o
//! sistema operacional, e nao por teste unitario -- a regra da casa. O cubo
//! em si (contar so replicas, o recuo que dobra) tem prova pura em
//! `src/quorum.rs`.
//!
//! As provas, e o defeito que cada uma derruba (teste 1..9 do contrato,
//! `docs/propostas/207-e-513p2-contrato-01-10-2026.md` §207.6):
//!
//! - `o_commit_espera_a_replica_e_ela_tem_a_linha_no_disco` -- 3 (o ack nao
//!   pede a trava), 4 (ack depois do `fsync`), 6 (o master sincroniza antes
//!   de esperar), e o «ok» que quer dizer «a replica TEM a linha»;
//! - `duas_replicas_e_depois_uma_e_depois_nenhuma` -- 1 (degrada dizendo, e
//!   o segundo commit nao espera), 2 (invisivel ate o ok), 8 (conta so
//!   replicas);
//! - `sem_quorum_a_resposta_e_a_de_sempre` -- 9 (o comportamento velho).
#![cfg(unix)]

mod comum;
use comum::{DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "quorum-207";

fn config_do_no(
    base: &Path,
    este: &str,
    papel: &str,
    portas: &[(String, u16)],
    quorum: u64,
    prazo_ms: u64,
) -> std::path::PathBuf {
    let porta_este = portas.iter().find(|(id, _)| id == este).unwrap().1;
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    let nos: Vec<String> = portas
        .iter()
        .map(|(id, p)| format!(r#"{{ "id": "{id}", "endereco": "127.0.0.1", "porta": {p} }}"#))
        .collect();
    let caminho = base.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:{porta_este}",
              "base": "{base_dir}",
              "token": "{TOKEN}",
              "log_acessos": "{log}",
              "seguranca": {{ "blacklist": "{bl}" }},
              "dblink": "{dblink}",
              "jobs": "{jobs}",
              "web": {{ "ligado": false }},
              "cifra_fio": {{ "exigir": false }},
              "replicacao": {{ "papel": "{papel}", "id_servidor": "{este}", "imagem_da_linha": true }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 30,
                "pulso_s": 1,
                "cifra": false,
                "quorum_minimo": {quorum},
                "quorum_prazo_ms": {prazo_ms},
                "nos": [ {nos} ]
              }}
            }}"#,
            base_dir = bar(base.join("base")),
            log = bar(base.join("acessos.log")),
            bl = bar(base.join("blacklist.json")),
            dblink = bar(base.join("dblink.json")),
            jobs = bar(base.join("jobs.json")),
            nos = nos.join(", "),
        ),
    )
    .unwrap();
    caminho
}

/// Um pedido numa conexao nova; devolve a resposta inteira.
fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    // Uma linha so: o protocolo e uma linha por pedido.
    let corpo = corpo.replace('\n', " ");
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    Json::analisar(&resposta).unwrap_or_else(|e| panic!("{e}: {resposta:?}"))
}

fn ok(r: Json) -> Json {
    assert!(r.booleano_ou("ok", false), "recusado: {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

struct Tres {
    _dirs: Vec<DirTemp>,
    _master: Arc<Servidor>,
    porta_master: u16,
    replicas: Vec<(String, u16, Option<Filho>)>,
}

impl Tres {
    fn subir(rotulo: &str, quorum: u64, prazo_ms: u64) -> Tres {
        let (ouvinte_m, porta_m) = comum::ouvinte_reservado();
        let (o2, p2) = comum::ouvinte_reservado();
        let (o3, p3) = comum::ouvinte_reservado();
        let portas = vec![
            ("no1".to_string(), porta_m),
            ("no2".to_string(), p2),
            ("no3".to_string(), p3),
        ];
        let d1 = DirTemp::novo(&format!("q207-{rotulo}-no1"));
        let d2 = DirTemp::novo(&format!("q207-{rotulo}-no2"));
        let d3 = DirTemp::novo(&format!("q207-{rotulo}-no3"));
        let c1 = config_do_no(&d1, "no1", "source", &portas, quorum, prazo_ms);
        let master = Servidor::novo(Config::ler(&c1).unwrap()).unwrap();
        comum::no_ar_no_ouvinte(&master, ouvinte_m);
        let mut replicas = Vec::new();
        for (id, d, ouvinte, porta) in [("no2", &d2, o2, p2), ("no3", &d3, o3, p3)] {
            let c = config_do_no(d, id, "replica", &portas, quorum, prazo_ms);
            let erro = d.join("stderr.txt");
            // O numero foi segurado ate aqui pelo ouvinte; solto agora, o
            // filho o pega no `bind` -- e a conferencia abaixo diz se pegou.
            drop(ouvinte);
            let mut filho = Filho(
                Command::new(env!("CARGO_BIN_EXE_phxsqld"))
                    .arg("--config")
                    .arg(&c)
                    .stdout(Stdio::null())
                    .stderr(Stdio::from(std::fs::File::create(&erro).unwrap()))
                    .spawn()
                    .unwrap(),
            );
            let real = comum::porta_do_phxsqld(&mut filho, &erro).unwrap();
            assert_eq!(real, porta, "a replica {id} nao abriu a porta da lista");
            replicas.push((id.to_string(), porta, Some(filho)));
        }
        Tres {
            _dirs: vec![d1, d2, d3],
            _master: master,
            porta_master: porta_m,
            replicas,
        }
    }

    fn estado_do_quorum(&self, porta: u16) -> Json {
        ok(pedir(porta, r#""op":"replicacao_estado""#))
            .campo("quorum")
            .cloned()
            .unwrap_or(Json::Nulo)
    }

    /// Espera as duas replicas conversarem pelo canal do quorum.
    fn esperar_as_replicas(&self, quantas: usize) {
        let ate = Instant::now() + Duration::from_secs(40);
        loop {
            let q = self.estado_do_quorum(self.porta_master);
            let n = match q.campo("replicas") {
                Some(Json::Objeto(p)) => p.len(),
                _ => 0,
            };
            if n >= quantas {
                return;
            }
            assert!(
                Instant::now() < ate,
                "as replicas nao abriram o canal do quorum em 40 s: {}",
                q.escrever()
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn matar(&mut self, id: &str) {
        let r = self.replicas.iter_mut().find(|(i, _, _)| i == id).unwrap();
        // `Filho::drop` manda SIGKILL e espera: e a queda que o SO ve.
        r.2.take();
    }

    fn inserir(&self, id: i64) -> (Json, f64) {
        let comeco = Instant::now();
        let r = ok(pedir(
            self.porta_master,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id}}}"#
            ),
        ));
        (r, comeco.elapsed().as_secs_f64() * 1000.0)
    }
}

fn preparar(t: &Tres) {
    ok(pedir(
        t.porta_master,
        r#""op":"criar_database","database":"loja""#,
    ));
    ok(pedir(
        t.porta_master,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    ));
}

/// Quantas linhas a tabela tem NESTE no -- pelo soquete dele.
fn linhas_em(porta: u16) -> usize {
    let r = pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":1000"#,
    );
    if !r.booleano_ou("ok", false) {
        return 0;
    }
    r.campo("resultado")
        .and_then(|x| x.campo("linhas").or(Some(x)))
        .and_then(Json::lista)
        .map_or(0, |l| l.len())
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// **O commit espera a replica, e quando volta ela TEM a linha, no disco.**
///
/// RED medidos (cada um com o defeito reposto, `bancada/guardas/catalogo.py`):
/// - `replicar_aguardar` tomando `travar_dados()` (`quorum-ack-pede-a-trava`):
///   a replica nao leva o lote enquanto o commit segura a trava, e o commit
///   volta pelo prazo com `alcancado:false`;
/// - sem o `sincronizar` antes do ack (`quorum-ack-antes-do-fsync`): os acks
///   saem sem arquivo nenhum ido ao disco;
/// - sem o `sincronizar` local (`quorum-sem-fsync-local`): o master espera
///   sem ter levado nada ao disco.
#[test]
fn o_commit_espera_a_replica_e_ela_tem_a_linha_no_disco() {
    let t = Tres::subir("ok", 1, 10_000);
    preparar(&t);
    t.esperar_as_replicas(2);
    let mut com = Vec::new();
    for i in 1..=30 {
        let (r, ms) = t.inserir(i);
        let q = r
            .campo("quorum")
            .unwrap_or_else(|| panic!("a resposta nao traz o quorum: {}", r.escrever()));
        assert!(
            q.booleano_ou("alcancado", false),
            "o insert {i} nao alcancou o quorum em {ms:.1} ms: {}",
            r.escrever()
        );
        assert!(q.inteiro_ou("confirmado", 0) >= 1);
        assert!(
            ms < 3_000.0,
            "o commit {i} levou {ms:.1} ms: o ack esperou alguma coisa que nao a replica"
        );
        com.push(ms);
        // «Confirmou» quer dizer que a linha JA esta numa replica: pergunta
        // as duas pelo soquete delas, logo depois da resposta.
        let tem = t
            .replicas
            .iter()
            .filter(|(_, p, _)| linhas_em(*p) >= i as usize)
            .count();
        assert!(
            tem >= 1,
            "insert {i} confirmado e nenhuma replica tem a linha"
        );
    }
    // Teste 4: todo ack mandado veio depois de um `fsync` que levou arquivo.
    for (id, porta, _) in &t.replicas {
        let q = t.estado_do_quorum(*porta);
        let acks = q.inteiro_ou("acks_mandados", 0);
        let com_fsync = q.inteiro_ou("acks_depois_do_fsync", 0);
        assert!(
            acks > 0,
            "a replica {id} nao confirmou nada: {}",
            q.escrever()
        );
        assert_eq!(
            acks,
            com_fsync,
            "a replica {id} confirmou sem levar arquivo ao disco: {}",
            q.escrever()
        );
    }
    // Teste 6: o master levou ao disco ANTES de esperar.
    let q = t.estado_do_quorum(t.porta_master);
    assert!(
        q.inteiro_ou("arquivos_sincronizados_antes_de_esperar", 0) >= 30,
        "o master esperou sem sincronizar: {}",
        q.escrever()
    );
    assert_eq!(q.texto_ou("estado", ""), "sincrono", "{}", q.escrever());

    // O custo, medido aqui mesmo: o MESMO insert com o quorum desligado a
    // quente (D-quente) -- e o desligado nao traz o campo.
    ok(pedir(
        t.porta_master,
        r#""op":"config_gravar","campos":{"cluster.quorum_minimo":0}"#,
    ));
    let mut sem = Vec::new();
    for i in 101..=130 {
        let (r, ms) = t.inserir(i);
        assert!(r.campo("quorum").is_none(), "{}", r.escrever());
        sem.push(ms);
    }
    eprintln!(
        "quorum 1-de-2 (loopback, debug): mediana {:.3} ms com quorum, {:.3} ms sem",
        mediana(com),
        mediana(sem)
    );
}

/// **Duas replicas, depois uma, depois nenhuma -- com `quorum_minimo:2`.**
///
/// 1. as duas vivas: confirma 2;
/// 2. `SIGKILL` numa: o commit volta no prazo com `alcancado:false`,
///    `confirmado:1` e `degradado:true` -- e a gravacao FICA. Durante a
///    espera, um leitor concorrente nao ve a linha antes de o commit
///    responder (teste 2);
/// 3. o commit seguinte, degradado, nao espera.
///
/// RED: contar o master (C1) faz o passo 2 alcancar; esperar sempre (sem o
/// degradado) faz o passo 3 levar o prazo inteiro; esperar depois de soltar
/// a trava faz o leitor ver a linha com o commit ainda esperando.
#[test]
fn duas_replicas_e_depois_uma_e_depois_nenhuma() {
    const PRAZO: u64 = 1_500;
    let mut t = Tres::subir("degrada", 2, PRAZO);
    preparar(&t);
    t.esperar_as_replicas(2);

    let (r, ms) = t.inserir(1);
    let q = r.campo("quorum").cloned().unwrap();
    assert!(
        q.booleano_ou("alcancado", false),
        "{} em {ms:.1} ms",
        r.escrever()
    );
    assert_eq!(q.inteiro_ou("confirmado", 0), 2);

    t.matar("no3");

    // O leitor concorrente: comeca no meio da espera do commit.
    let porta = t.porta_master;
    let comeco = Instant::now();
    let leitor = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        let n = linhas_em(porta);
        (n, comeco.elapsed())
    });
    let (r, ms) = t.inserir(2);
    let commit_voltou = comeco.elapsed();
    let (vistas, leitor_voltou) = leitor.join().unwrap();
    let q = r.campo("quorum").cloned().unwrap();
    assert!(!q.booleano_ou("alcancado", true), "{}", r.escrever());
    assert_eq!(q.inteiro_ou("confirmado", -1), 1, "{}", r.escrever());
    assert!(q.booleano_ou("degradado", false), "{}", r.escrever());
    assert!(
        r.campo("aviso_quorum").is_some(),
        "a espera vencida tem de vir DITA: {}",
        r.escrever()
    );
    assert!(
        ms >= PRAZO as f64 * 0.9 && ms < PRAZO as f64 + 3_000.0,
        "o commit tinha de esperar o prazo ({PRAZO} ms) e voltar: {ms:.1} ms"
    );
    if vistas >= 2 {
        assert!(
            leitor_voltou + Duration::from_millis(50) >= commit_voltou,
            "o leitor viu a linha 2 em {leitor_voltou:?}, ANTES de o commit \
             responder em {commit_voltou:?}"
        );
    }
    assert_eq!(linhas_em(t.porta_master), 2, "a gravacao fica (D-falha)");

    // Degradado: o seguinte nao espera.
    let (r, ms) = t.inserir(3);
    let q = r.campo("quorum").cloned().unwrap();
    assert!(q.booleano_ou("degradado", false), "{}", r.escrever());
    assert!(
        ms < 250.0,
        "degradado, o commit nao pode esperar: {ms:.1} ms ({})",
        r.escrever()
    );
    let e = t.estado_do_quorum(t.porta_master);
    assert_eq!(e.texto_ou("estado", ""), "degradado", "{}", e.escrever());
    assert!(e.inteiro_ou("degradacoes", 0) >= 1);
}

/// **Sem quorum, a resposta e byte a byte a de sempre** -- o teste do
/// comportamento VELHO, que e o que importa numa guarda nova (pedida, nao
/// imposta).
#[test]
fn sem_quorum_a_resposta_e_a_de_sempre() {
    let t = Tres::subir("zero", 0, 10_000);
    preparar(&t);
    let (r, ms) = t.inserir(1);
    assert!(r.campo("quorum").is_none(), "{}", r.escrever());
    assert!(r.campo("aviso_quorum").is_none());
    assert!(ms < 3_000.0);
    let q = t.estado_do_quorum(t.porta_master);
    assert_eq!(q.texto_ou("estado", ""), "desligado", "{}", q.escrever());
    assert_eq!(q.inteiro_ou("esperas", -1), 0);
}

/// Uma conexao que FICA -- a transacao e da sessao, e cada `pedir` acima
/// abre uma nova.
struct Conexao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Conexao {
    fn nova(porta: u16) -> Conexao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        Conexao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut resposta = String::new();
        self.leitor.read_line(&mut resposta).unwrap();
        Json::analisar(&resposta).unwrap_or_else(|e| panic!("{e}: {resposta:?}"))
    }
}

/// **Toda familia de escrita traz o `quorum`** -- a guarda
/// `quorum-escritor-sem-espera`. A anotacao mora no PONTO UNICO onde o
/// diario cresce (`LogFile::registrar_detalhado`), e nao em cada familia;
/// esta prova e o que diz que o ponto e mesmo unico para as que existem hoje.
///
/// RED: tirar a anotacao do `log.rs` -- nenhuma familia traz o campo.
#[test]
fn cada_familia_de_escrita_traz_o_quorum() {
    let t = Tres::subir("familias", 1, 10_000);
    preparar(&t);
    t.esperar_as_replicas(2);
    let alcancou = |rotulo: &str, r: &Json| {
        let q = r
            .campo("quorum")
            .unwrap_or_else(|| panic!("{rotulo}: a resposta nao traz o quorum: {}", r.escrever()));
        assert!(
            q.booleano_ou("alcancado", false),
            "{rotulo}: {}",
            r.escrever()
        );
    };
    let p = t.porta_master;
    let r = ok(pedir(
        p,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1}"#,
    ));
    alcancou("inserir", &r);
    let rowid = r.inteiro_ou("rowid", 1);
    let r = ok(pedir(
        p,
        r#""op":"inserir_lote","database":"loja","tabela":"clientes","linhas":[{"id":2},{"id":3},{"id":4}]"#,
    ));
    alcancou("inserir_lote", &r);
    let r = ok(pedir(
        p,
        &format!(
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":{rowid},"valores":{{"id":10}}"#
        ),
    ));
    alcancou("atualizar", &r);
    let r = ok(pedir(
        p,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":2,"motivo":"teste do quorum""#,
    ));
    alcancou("excluir", &r);
    let r = ok(pedir(
        p,
        r#""op":"sql","database":"loja","texto":"INSERT INTO clientes (id) VALUES (50)""#,
    ));
    alcancou("sql INSERT", &r);
    // A transacao: o COMMIT e quem grava, e e ele que espera.
    let mut c = Conexao::nova(p);
    ok(c.pedir(r#""op":"begin""#));
    let r =
        ok(c.pedir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":60}"#));
    assert!(
        r.campo("quorum").is_none(),
        "empilhar nao grava, e nao espera: {}",
        r.escrever()
    );
    let r = ok(c.pedir(r#""op":"commit""#));
    alcancou("commit", &r);
    // A carga reservada: cada lote grava, e cada lote espera.
    let mut c = Conexao::nova(p);
    ok(c.pedir(r#""op":"bulkinsert","database":"loja","tabela":"clientes","ligado":true"#));
    let r = ok(c.pedir(
        r#""op":"inserir_lote","database":"loja","tabela":"clientes","linhas":[{"id":70},{"id":71}]"#,
    ));
    alcancou("lote do bulkinsert", &r);
    ok(c.pedir(r#""op":"bulkinsert","database":"loja","tabela":"clientes","ligado":false"#));
}
