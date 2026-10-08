//! Pedido 715 (F5, reescrito pelo papel C): a recuperacao completa as marcas
//! na ordem em que a TRAVA as teria aplicado, e nao na do id -- provado contra
//! o sistema operacional, com `SIGKILL`.
//!
//! # O defeito
//!
//! A marca do grupo da replica (`transacao_`, v5/v6) nasce FORA da trava e
//! espera por ela; a do `COMMIT` local nasce COM a trava, e o id dela e o da
//! transacao, tirado no `BEGIN`. O `BEGIN` que vem depois da marca do grupo
//! tem id MAIOR, toma a trava ANTES do grupo e morre no meio da passada. Ao
//! vivo o `COMMIT` entra inteiro e o grupo para na ruptura; o arranque
//! completava pelo id -- o grupo primeiro --, o grupo tomava o rowid que o
//! `COMMIT` tinha planejado, e o `COMMIT` saia pela metade dado como
//! completado.
#![cfg(unix)]

mod comum;
use comum::{pedir, porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::value::Value;
use phxsql_server::{Config, Papel, Servidor};
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "ordem-da-recuperacao";
const ESPERA: Duration = Duration::from_secs(30);
const PARAR_ANTES_DA_TRAVA: &str = "PHXSQL_TESTE_PARAR_ANTES_DA_TRAVA_DO_GRUPO";
const PARAR_NO_REG_DO_COMMIT: &str = "PHXSQL_TESTE_PARAR_NO_REG_DO_COMMIT";

fn subir_origem(base: &Path) -> (Arc<Servidor>, u16) {
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
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "caixa01".into();
    c.replicacao.imagem_da_linha = true;
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    (s, porta)
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Option<Json> {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
        let mut r = String::new();
        match self.leitor.read_line(&mut r) {
            Ok(n) if n > 0 => Some(Json::analisar(&r).unwrap()),
            _ => None,
        }
    }

    fn exigir(&mut self, corpo: &str) {
        let r = self.pedir(corpo).expect("a conexao caiu sem resposta");
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    }
}

fn inserir(tabela: &str, id: u64, venda: &str) -> String {
    format!(
        r#""op":"inserir","database":"loja","tabela":"{tabela}","linha":{{"id":{id},"venda":"{venda}"}}"#
    )
}

/// A replica SEM espelho e sem somente-leitura: a escrita local no database
/// recebido e permitida (a guarda do 677 entra pedida).
fn subir_replica(dir: &Path, vez: u32, porta_origem: u16, ganchos: bool) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "replicacao": {{
                    "papel": "replica", "id_servidor": "central",
                    "imagem_da_linha": true,
                    "origens": [{{ "nome": "caixa01", "host": "127.0.0.1",
                                   "porta": {porta_origem}, "token": "{TOKEN}",
                                   "databases": ["loja"], "reconectar_em": 1 }}]
                }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join(format!("stderr-{vez}.txt"));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phxsqld"));
    cmd.arg("--config")
        .arg(&config)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
        .env_remove(PARAR_ANTES_DA_TRAVA)
        .env_remove(PARAR_NO_REG_DO_COMMIT);
    if ganchos {
        cmd.env(PARAR_ANTES_DA_TRAVA, "2")
            .env(PARAR_NO_REG_DO_COMMIT, "1");
    }
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

fn contar(porta: u16, tabela: &str) -> usize {
    let r = pedir(
        porta,
        &format!(
            r#"{{"token":"{TOKEN}","op":"varrer","database":"loja","tabela":"{tabela}","max":5000}}"#
        ),
    );
    Json::analisar(&r)
        .ok()
        .and_then(|j| j.campo("resultado").cloned())
        .and_then(|r| r.campo("linhas").and_then(Json::lista).map(<[Json]>::len))
        .unwrap_or(0)
}

fn esperar_no_erro(arquivo: &Path, texto: &str) {
    let ate = Instant::now() + ESPERA;
    while !std::fs::read_to_string(arquivo)
        .unwrap_or_default()
        .contains(texto)
    {
        assert!(Instant::now() < ate, "nunca apareceu {texto:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A linha 2 de `t` no disco: o `venda` dela.
fn linha_2_de_t(dados: &Path) -> Option<String> {
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let mut t = db.abrir_qualificada("t").unwrap();
    if t.slots() < 2 {
        return None;
    }
    t.ler(2).unwrap().map(|l| match &l[1] {
        Value::Str(s) => s.clone(),
        outro => format!("{outro:?}"),
    })
}

/// O roteiro: a origem manda `t` e `u` (grupo 1, entra) e depois a linha 2
/// de `t` (grupo 2, PARA com a marca no disco e sem a trava). Na replica, um
/// `COMMIT` local insere em `u` e em `t` e morre dentro da primeira inclusao.
/// `begin_antes` abre a transacao ANTES da marca do grupo (id menor).
fn roteiro(rotulo: &str, begin_antes: bool) -> Option<String> {
    let base_o = DirTemp::novo(&format!("{rotulo}-origem"));
    let base_c = DirTemp::novo(&format!("{rotulo}-central"));
    let (_origem, porta_o) = subir_origem(&base_o.0);
    let mut o = Ligacao::nova(porta_o);
    o.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["t", "u"] {
        o.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Str(20)"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
        ));
    }
    o.exigir(r#""op":"begin","database":"loja""#);
    o.exigir(&inserir("t", 1, "o1"));
    o.exigir(&inserir("u", 1, "o1"));
    o.exigir(r#""op":"commit""#);

    let (filho, porta_c) = subir_replica(&base_c.0, 1, porta_o, true);
    let ate = Instant::now() + ESPERA;
    while contar(porta_c, "t") < 1 || contar(porta_c, "u") < 1 {
        assert!(Instant::now() < ate, "o grupo 1 nao chegou");
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut c = Ligacao::nova(porta_c);
    if begin_antes {
        c.exigir(r#""op":"begin","database":"loja""#);
    }
    o.exigir(&inserir("t", 2, "o2"));
    esperar_no_erro(
        &base_c.0.join("stderr-1.txt"),
        "teste: grupo da replica parado antes da trava",
    );
    if !begin_antes {
        c.exigir(r#""op":"begin","database":"loja""#);
    }
    c.exigir(&inserir("u", 20, "c"));
    c.exigir(&inserir("t", 20, "c"));
    assert!(
        c.pedir(r#""op":"commit""#).is_none(),
        "o COMMIT respondeu: o gancho nao matou o processo"
    );
    esperar_no_erro(
        &base_c.0.join("stderr-1.txt"),
        "teste: COMMIT parado entre o .reg e o diario",
    );
    drop(c);
    drop(filho);

    let (filho, _) = subir_replica(&base_c.0, 2, comum::porta_fechada(), false);
    drop(filho);
    linha_2_de_t(&base_c.0.join("dados"))
}

/// **A prova do 715 (F5).** O `BEGIN` depois da marca do grupo (id maior)
/// entra antes dele na trava. Vermelho medido antes do conserto: o arranque
/// completa o grupo primeiro, e a linha 2 de `t` sai a da ORIGEM -- o `COMMIT`
/// local perde a inclusao em `t`.
#[test]
fn o_commit_que_entrou_antes_do_grupo_e_completado_antes_dele() {
    assert_eq!(
        roteiro("f5", false).as_deref(),
        Some("c"),
        "o arranque completou o grupo da replica antes do COMMIT que a trava aplicou primeiro"
    );
}

/// O controle: o `BEGIN` antes da marca do grupo (id menor) ja sai certo.
#[test]
fn controle_o_commit_de_id_menor_tambem_entra_primeiro() {
    assert_eq!(roteiro("f5-controle", true).as_deref(), Some("c"));
}
