//! Pedido 620: o mapa de toques do bidirecional e da VIDA da tabela, e nao do
//! nome -- provado PELO SOQUETE, com dois servidores multi de verdade.
//!
//! # O defeito
//!
//! O mapa guarda ate onde o diario local ja foi absorvido (`vistos`) e a marca
//! de onde a leitura parou no `.log`. A tabela apagada e recriada -- ou
//! restaurada de um backup -- tem OUTRO `.log` no mesmo caminho, e o mapa nao
//! sabia: com o diario novo menor que o `vistos` velho, nada da vida nova
//! entrava no mapa; maior, a marca velha apontava para o meio (ou para alem do
//! fim) do arquivo novo. Nos dois casos a escrita local nova ficava fora do
//! mapa, e o «mais recente vence» deixava um evento remoto MAIS VELHO
//! sobrescreve-la -- dado errado, calado.
//!
//! # Como a ordem fica determinada, sem depender da rodada de 1 s
//!
//! B puxa de A por uma COMPORTA do proprio teste: um repasse TCP que o teste
//! fecha e abre. Com ela fechada, B nao consegue rodar -- e e nesse intervalo
//! que A escreve a chave 1 (mais velha) e B recria a tabela e escreve a mesma
//! chave (mais nova). Aberta, o evento de A chega DEPOIS da escrita de B, sempre.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::mcp::Executor;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "toques-de-outra-vida";
const ESPERA: Duration = Duration::from_secs(30);

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = tentar(self.porta, r#""op":"servico_parar""#);
    }
}

fn config(base: &std::path::Path, id: &str) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O ESCAPE ESCRITO: o que esta bateria mede e o mapa de toques, nao o
    // portao da cifra do fio.
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c
}

fn subir(c: Config) -> NoAr {
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().ok()?;
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

/// Por onde o teste fala com B. A restauracao POR CIMA exige a porta de dados
/// parada, e dai em diante B so se alcanca pelo executor local -- o mesmo
/// `despachar` da porta, sem soquete. O resto do cenario segue pela porta.
enum Lado {
    Porta(u16),
    Local(ExecutorLocal),
}

impl Lado {
    /// O `resultado` do pedido, ou o texto do erro.
    fn pedir(&self, corpo: &str) -> Result<Json, String> {
        match self {
            Lado::Porta(porta) => {
                let r = tentar(*porta, corpo).ok_or_else(|| format!("sem resposta de {porta}"))?;
                if r.booleano_ou("ok", false) {
                    Ok(r.campo("resultado").cloned().unwrap_or(Json::Nulo))
                } else {
                    Err(r.escrever())
                }
            }
            Lado::Local(e) => {
                let linha = format!("{{\"token\":\"{TOKEN}\",{}}}", corpo.replace('\n', " "));
                let pedido = Json::analisar(&linha).map_err(|e| e.to_string())?;
                e.executar(&pedido).map_err(|e| e.to_string())
            }
        }
    }

    fn exigir(&self, corpo: &str) -> Json {
        self.pedir(corpo)
            .unwrap_or_else(|e| panic!("{corpo} -> {e}"))
    }
}

fn esperar<F: FnMut() -> bool>(o_que: &str, mut f: F) {
    let ate = Instant::now() + ESPERA;
    while Instant::now() < ate {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{o_que} nao aconteceu em {} s", ESPERA.as_secs());
}

// ------------------------------------------------------------- a comporta

/// Um repasse TCP que o teste abre e fecha. Fechada, ela derruba as conexoes
/// vivas e recusa as novas -- e conta as recusadas, que e a prova de que o
/// laco de B ja saiu da rodada em que estava.
struct Comporta {
    porta: u16,
    aberta: Arc<AtomicBool>,
    recusadas: Arc<AtomicU64>,
    vivas: Arc<Mutex<Vec<TcpStream>>>,
}

impl Comporta {
    fn para(destino: u16) -> Comporta {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let aberta = Arc::new(AtomicBool::new(true));
        let recusadas = Arc::new(AtomicU64::new(0));
        let vivas: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        let (a, r, v) = (
            Arc::clone(&aberta),
            Arc::clone(&recusadas),
            Arc::clone(&vivas),
        );
        std::thread::spawn(move || {
            for entrada in ouvinte.incoming() {
                let Ok(entrada) = entrada else { continue };
                if !a.load(Ordering::SeqCst) {
                    r.fetch_add(1, Ordering::SeqCst);
                    let _ = entrada.shutdown(Shutdown::Both);
                    continue;
                }
                let Ok(saida) = TcpStream::connect(("127.0.0.1", destino)) else {
                    continue;
                };
                {
                    let mut g = v.lock().unwrap();
                    g.push(entrada.try_clone().unwrap());
                    g.push(saida.try_clone().unwrap());
                }
                let (mut e1, mut s1) = (entrada.try_clone().unwrap(), saida.try_clone().unwrap());
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut e1, &mut s1);
                    let _ = s1.shutdown(Shutdown::Both);
                    let _ = e1.shutdown(Shutdown::Both);
                });
                let (mut e2, mut s2) = (entrada, saida);
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut s2, &mut e2);
                    let _ = s2.shutdown(Shutdown::Both);
                    let _ = e2.shutdown(Shutdown::Both);
                });
            }
        });
        Comporta {
            porta,
            aberta,
            recusadas,
            vivas,
        }
    }

    /// Fecha e so volta quando o laco de B ja bateu na comporta fechada: dali
    /// em diante nenhuma rodada esta no meio.
    fn fechar(&self) {
        self.aberta.store(false, Ordering::SeqCst);
        for s in self.vivas.lock().unwrap().drain(..) {
            let _ = s.shutdown(Shutdown::Both);
        }
        let antes = self.recusadas.load(Ordering::SeqCst);
        esperar("o laco de B bater na comporta fechada", || {
            self.recusadas.load(Ordering::SeqCst) > antes
        });
    }

    fn abrir(&self) {
        self.aberta.store(true, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- o terreno

fn criar_clientes(l: &Lado) {
    l.exigir(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(30)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

fn inserir(l: &Lado, id: i64, nome: &str) {
    l.exigir(&format!(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id},"nome":"{nome}"}}"#
    ));
}

fn nome_de(l: &Lado, id: i64) -> Option<String> {
    l.pedir(&format!(
        r#""op":"buscar","database":"loja","tabela":"clientes","indice":"porId","chave":[{id}]"#
    ))
    .ok()?
    .campo("linhas")
    .and_then(Json::lista)
    .and_then(|l| l.first())
    .map(|l| l.texto_ou("nome", "").to_string())
}

fn do_mapa(l: &Lado) -> Json {
    l.exigir(r#""op":"replicacao_estado""#)
        .campo("toques_no_mapa")
        .and_then(|m| m.campo("loja/clientes"))
        .cloned()
        .unwrap_or(Json::Nulo)
}

fn colisoes(l: &Lado) -> i64 {
    l.exigir(r#""op":"replicacao_estado""#)
        .campo("colisoes_de_sequencia")
        .and_then(|c| c.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(0)
}

/// O que se faz em B com a comporta fechada, ANTES da escrita local nova na
/// chave 1.
#[derive(Clone, Copy)]
enum VidaNova {
    /// Nada: a mesma vida da tabela -- o comportamento velho.
    Nenhuma,
    /// `excluir_tabela` + `criar_tabela`, e so a chave 1 depois: o diario
    /// novo fica MENOR que o `vistos` velho.
    Recriada,
    /// Recriada, e a chave 1 seguida de `extras` linhas: o diario novo passa
    /// do `vistos` velho, e a marca velha aponta para o arquivo errado.
    RecriadaMaior(i64),
    /// `restaurar_backup` por cima de um retrato antigo do proprio B.
    Restaurada,
}

/// O cenario inteiro. Devolve `(nome da chave 1 em B, o mapa de B)`.
fn cenario(rotulo: &str, vida: VidaNova) -> (String, Json) {
    let dir_a = DirTemp::novo(&format!("outra-vida-a-{rotulo}"));
    let dir_b = DirTemp::novo(&format!("outra-vida-b-{rotulo}"));
    let copias = dir_b.join("copias");
    let base_b = dir_b.join("dados");
    let a = subir(config(&dir_a, "vida-a"));
    let la = Lado::Porta(a.porta);
    la.exigir(r#""op":"criar_database","database":"loja""#);
    criar_clientes(&la);
    inserir(&la, 50, "de A, da vida velha");

    let comporta = Comporta::para(a.porta);
    let mut cb = config(&base_b, "vida-b");
    cb.replicacao.origens = vec![Origem {
        nome: "a".into(),
        host: "127.0.0.1".into(),
        porta: comporta.porta,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
    }];
    let b = subir(cb);
    let mut lb = Lado::Porta(b.porta);
    esperar("a linha 50 de A em B", || nome_de(&lb, 50).is_some());

    // A vida velha de B: linhas locais suficientes para o `vistos` passar,
    // de longe, o tamanho do diario da vida nova.
    let mut zip = String::new();
    for id in 100..120 {
        inserir(&lb, id, "de B, vida velha, linha longa");
        if id == 102 {
            let r = lb.exigir(&format!(
                r#""op":"backup","destino":"{}","database":"loja","zip":true"#,
                copias.display()
            ));
            zip = r.texto_ou("arquivo", "").to_string();
        }
    }
    esperar("o mapa de B absorver a vida velha", || {
        do_mapa(&lb).inteiro_ou("vistos", 0) >= 21
    });

    comporta.fechar();
    // A escreve PRIMEIRO -- e a escrita mais velha.
    inserir(&la, 1, "de A, mais velha");
    std::thread::sleep(Duration::from_millis(20));
    match vida {
        VidaNova::Nenhuma => {}
        VidaNova::Recriada | VidaNova::RecriadaMaior(_) => {
            lb.exigir(
                r#""op":"excluir_tabela","database":"loja","tabela":"clientes","confirmar":"clientes""#,
            );
            criar_clientes(&lb);
        }
        VidaNova::Restaurada => {
            assert!(!zip.is_empty(), "o backup de B nao devolveu o arquivo");
            lb.exigir(r#""op":"servico_parar""#);
            lb = Lado::Local(ExecutorLocal::novo(Arc::clone(&b._s), "teste"));
            let pedido = format!(
                r#""op":"restaurar_backup","origem":"{zip}","database":"loja","modo":"por_cima","confirmar":true"#
            );
            // A parada da porta e assincrona: o laco de aceitar sai no passo
            // seguinte dele.
            let mut feito = false;
            esperar("a restauracao por cima ser aceita", || {
                match lb.pedir(&pedido) {
                    Ok(_) => feito = true,
                    Err(e) => assert!(e.contains("pare a porta"), "{e}"),
                }
                feito
            });
        }
    }
    inserir(&lb, 1, "de B, mais nova");
    if let VidaNova::RecriadaMaior(extras) = vida {
        for id in 0..extras {
            inserir(&lb, 200 + id, "de B, vida nova");
        }
    }
    comporta.abrir();

    // O evento de A na chave 1 chegou quando: ou sobrescreveu (o defeito), ou
    // foi contado como colisao contra o toque de B (o conserto -- a inclusao
    // de A sobre a chave viva de B).
    esperar("o evento de A na chave 1 chegar em B", || {
        nome_de(&lb, 1).as_deref() == Some("de A, mais velha") || colisoes(&lb) >= 1
    });
    let nome = nome_de(&lb, 1).unwrap_or_default();
    (nome, do_mapa(&lb))
}

/// **Prova real, recriada com o diario novo MENOR que o `vistos` velho.**
///
/// **Defeito reposto** (o `absorver_diario_local` sem `recomecar_a_vida`): o
/// `vistos` de 21 nao deixa o evento local unico da vida nova entrar no mapa,
/// a chave 1 fica sem toque, o evento MAIS VELHO de A vence e o teste cai na
/// asercao do nome.
#[test]
fn recriada_a_escrita_local_nova_vence_o_evento_remoto_mais_velho() {
    let (nome, mapa) = cenario("menor", VidaNova::Recriada);
    assert_eq!(
        nome,
        "de B, mais nova",
        "o evento MAIS VELHO de A sobrescreveu a escrita nova de B (mapa: {})",
        mapa.escrever()
    );
    assert!(
        mapa.inteiro_ou("vidas_novas", 0) >= 1,
        "{}",
        mapa.escrever()
    );
}

/// **Prova real, recriada com o diario novo MAIOR que o `vistos` velho** --
/// a marca velha aponta para o `.log` errado.
///
/// **Defeito reposto** (idem): a absorcao comeca no evento 21 da vida nova e
/// a chave 1, o evento 0, nunca entra no mapa.
#[test]
fn recriada_e_crescida_a_escrita_local_nova_vence_o_evento_remoto_mais_velho() {
    let (nome, mapa) = cenario("maior", VidaNova::RecriadaMaior(30));
    assert_eq!(
        nome,
        "de B, mais nova",
        "o evento MAIS VELHO de A sobrescreveu a escrita nova de B (mapa: {})",
        mapa.escrever()
    );
    assert!(
        mapa.inteiro_ou("vidas_novas", 0) >= 1,
        "{}",
        mapa.escrever()
    );
}

/// **O irmao: restaurada de um backup antigo.** O `.log` restaurado e um
/// prefixo do velho, menor que o `vistos`.
///
/// **Defeito reposto** (idem): a chave 1 escrita depois da restauracao fica
/// fora do mapa e o evento mais velho de A vence.
#[test]
fn restaurada_a_escrita_local_nova_vence_o_evento_remoto_mais_velho() {
    let (nome, mapa) = cenario("restaurada", VidaNova::Restaurada);
    assert_eq!(
        nome,
        "de B, mais nova",
        "o evento MAIS VELHO de A sobrescreveu a escrita nova de B (mapa: {})",
        mapa.escrever()
    );
    assert!(
        mapa.inteiro_ou("vidas_novas", 0) >= 1,
        "{}",
        mapa.escrever()
    );
}

/// **O comportamento velho.** A mesma tabela, a mesma vida: a escrita nova de
/// B vence como sempre venceu, e o mapa NAO recomeca -- recomecar a cada
/// rodada seria varrer o diario inteiro a cada segundo.
#[test]
fn na_mesma_vida_o_mapa_nao_recomeca_e_decide_como_sempre() {
    let (nome, mapa) = cenario("mesma", VidaNova::Nenhuma);
    assert_eq!(nome, "de B, mais nova", "{}", mapa.escrever());
    assert_eq!(mapa.inteiro_ou("vidas_novas", -1), 0, "{}", mapa.escrever());
}
