//! Pedidos 765/767, fatia P1: a camada de protecao pela porta REST, de
//! verdade -- soquete, HTTP, `Bearer`.
//!
//! O REST entra pelo mesmo `despachar` da porta de dados, e e por isso que a
//! camada no `executar_e_contar_escrita_local` o alcanca. Esta prova existe
//! para que uma porta HTTP que um dia despache por outro caminho apareca
//! aqui, e nao no dia do estrago. O RED e a mesma corrida com
//! `protecao.ligada = false`: a tabela some.

mod comum;
use comum::DirTemp;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico desta prova";

fn subir(base: &Path, ligada: bool) -> Arc<Servidor> {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    let caminho = base.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0",
              "base": "{}",
              "token": "{TOKEN}",
              "log_acessos": "{}",
              "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}",
              "jobs": "{}",
              "cifra_fio": {{ "exigir": false }},
              "rest": {{ "ligado": true, "bind": "127.0.0.1:0" }},
              "protecao": {{ "ligada": {ligada} }}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert_eq!(c.protecao.ligada, ligada);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    s
}

/// `POST /v1/<op>`; devolve o status HTTP e o corpo.
fn rest(porta: u16, op: &str, corpo: &str) -> (u16, Json) {
    let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    write!(
        c,
        "POST /v1/{op} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n\
         Authorization: Bearer {TOKEN}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .unwrap();
    let mut v = Vec::new();
    let _ = c.read_to_end(&mut v);
    let r = String::from_utf8_lossy(&v).into_owned();
    let status = r
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let corpo = r.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
    let j = Json::analisar(corpo).unwrap_or_else(|e| panic!("nao e JSON ({e}): {r}"));
    (status, j)
}

fn corrida(rotulo: &str, ligada: bool) -> (u16, Json, bool) {
    let d = DirTemp::novo(rotulo);
    let s = subir(&d.0, ligada);
    let porta = comum::porta_real(|| s.porta_rest());
    let (st, r) = rest(porta, "criar_database", r#"{"database":"b"}"#);
    assert_eq!(st, 200, "{}", r.escrever());
    let (st, r) = rest(
        porta,
        "criar_tabela",
        r#"{"database":"b","tabela":"c","colunas":[{"nome":"id","tipo":"Int4"}]}"#,
    );
    assert_eq!(st, 200, "{}", r.escrever());
    let (st, r) = rest(
        porta,
        "excluir_tabela",
        r#"{"database":"b","tabela":"c","confirmar":"c"}"#,
    );
    let existe = d.0.join("base").join("b").join("c.reg").exists();
    (st, r, existe)
}

#[test]
fn o_drop_pelo_rest_e_recusado_com_a_camada_e_executa_sem_ela() {
    let (st, r, existe) = corrida("protecao-rest-ligada", true);
    assert_eq!(st, 403, "{}", r.escrever());
    assert_eq!(r.inteiro_ou("codigo", 0), 4009, "{}", r.escrever());
    assert_eq!(r.texto_ou("nome", ""), "SENHA_DE_EXECUCAO_EXIGIDA");
    assert!(existe, "a tabela sumiu apesar da recusa");

    let (st, r, existe) = corrida("protecao-rest-desligada", false);
    assert_eq!(st, 200, "{}", r.escrever());
    assert!(!existe, "sem a camada, o DROP pelo REST tinha de executar");
}
