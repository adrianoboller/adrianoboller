//! TLS 1.3 nativo na porta de DADOS (pedido 572, T6) -- pelo soquete.
//!
//! 1. com `"tls": true` e a cifra exigida de fabrica, um cliente TLS
//!    (`openssl s_client`, conferindo o certificado) fala JSON Lines e e
//!    ATENDIDO. Repor o defeito: tirar o `!saida.cifrado()` da exigencia
//!    volta a recusar;
//! 2. na mesma porta, o cliente em claro continua sendo o de sempre -- e
//!    com a exigencia, recusado como sempre foi (a porta nao abriu um fundo);
//! 3. pediu TLS com par trocado: a porta NAO sobe.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";

fn config(base: &Path, extra: &str) -> Config {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "base": "{}", "token": "{TOKEN}",
              "log_acessos": "{}", "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}", "jobs": "{}"{extra}
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
    c
}

fn subir(base: &Path) -> (Arc<Servidor>, u16) {
    let c = config(base, r#", "tls": true"#);
    assert!(c.cifra_fio.exigir, "a prova e com a exigencia de fabrica");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

/// Uma linha de pedido pelo `openssl s_client`, e a primeira linha JSON que
/// voltar.
fn pedido_tls(porta: u16, ca: &Path, linha: &str) -> String {
    let mut filho = Command::new("openssl")
        .args(["s_client", "-connect", &format!("127.0.0.1:{porta}")])
        .args(["-tls1_3", "-quiet", "-verify_return_error", "-CAfile"])
        .arg(ca)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut entrada = filho.stdin.take().unwrap();
    entrada.write_all(format!("{linha}\n").as_bytes()).unwrap();
    let saida = filho.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(saida).lines().map_while(Result::ok) {
            if l.starts_with('{') {
                let _ = tx.send(l);
                return;
            }
        }
    });
    let resposta = rx.recv_timeout(Duration::from_secs(20)).unwrap_or_default();
    let _ = filho.kill();
    let _ = filho.wait();
    drop(entrada);
    resposta
}

fn ping() -> String {
    format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#)
}

#[test]
fn cliente_tls_fala_json_lines_com_a_cifra_exigida() {
    let d = DirTemp::novo("tls-dados");
    let (_s, porta) = subir(&d.0);
    let ca = d.0.join("tls-dados-certificado.pem");
    assert!(ca.exists(), "o autoassinado da porta de dados nao nasceu");
    let r = pedido_tls(porta, &ca, &ping());
    assert!(
        r.contains(r#""ok":true"#),
        "o ping por TLS nao foi atendido: {r:?}"
    );
}

#[test]
fn na_mesma_porta_o_claro_continua_recusado_pela_exigencia() {
    let d = DirTemp::novo("tls-dados-claro");
    let (_s, porta) = subir(&d.0);
    let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    writeln!(c, "{}", ping()).unwrap();
    let mut linha = String::new();
    BufReader::new(&c).read_line(&mut linha).unwrap();
    assert!(linha.contains(r#""ok":false"#), "o claro passou: {linha}");
    assert!(
        linha.contains("cifra"),
        "a recusa nao diz o motivo: {linha}"
    );
}

#[test]
fn par_trocado_nao_deixa_a_porta_subir() {
    // O par da porta de dados nasce num diretorio, o da web em outro, e a
    // config passa a apontar o certificado de um com a chave do outro.
    let d = DirTemp::novo("tls-dados-trocado");
    let (_s, _) = subir(&d.0);
    let outra = DirTemp::novo("tls-dados-trocado-2");
    let cw = config(
        &outra.0,
        r#", "web": {"ligado": true, "bind": "127.0.0.1:0", "tls": true}"#,
    );
    let sw = Servidor::novo(cw).unwrap();
    let copia = Arc::clone(&sw);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    comum::porta_real(|| sw.porta_web());
    std::fs::copy(
        outra.0.join("tls-web-chave.pem"),
        d.0.join("outra-chave.pem"),
    )
    .unwrap();

    let c = config(
        &d.0,
        r#", "tls": true, "tls_certificado": "tls-dados-certificado.pem",
           "tls_chave": "outra-chave.pem""#,
    );
    // Numa thread e com prazo: se a porta SUBIR, o `escutar` nunca volta, e
    // o teste tem de falhar dizendo isso em vez de travar a bateria.
    let s = Servidor::novo(c).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(s.escutar().map_err(|e| e.to_string()));
    });
    let e = match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Err(e)) => e,
        _ => panic!("a porta subiu com a chave de outro certificado"),
    };
    assert!(e.contains("nao e a do certificado"), "{e}");
}
