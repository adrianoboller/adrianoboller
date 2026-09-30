//! TLS 1.3 nativo nas portas HTTP (pedido 572, T4b) -- provado PELO SOQUETE,
//! com o `curl` conferindo o certificado.
//!
//! O que cada prova segura:
//!
//! 1. com `"tls": true` e a cifra exigida (o padrao desde o pedido 370), as
//!    tres portas HTTP ATENDEM -- o login abre sessao pela web, o `/v1` e o
//!    explorador respondem -- sem `atras_de_proxy` nenhum. Repor o defeito:
//!    tirar o `!fluxo.cifrado()` do `portao_de_rede_http` volta a recusa 403;
//! 2. a porta TLS nao responde em claro: quem fala HTTP cru recebe um alerta
//!    TLS, e nenhum byte de HTTP -- nunca cai calada para o texto puro;
//! 3. o autoassinado gerado na primeira subida e o MESMO na segunda (quem
//!    confiou nele uma vez continua confiando), e a chave so o dono le.

mod comum;
use comum::DirTemp;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";
const LOGIN: &str = "ana";
const SENHA: &str = "segredo123";

struct Portas {
    web: u16,
    rest: u16,
    swagger: u16,
}

/// Sobe as tres portas HTTP com `"tls": true` e SEM dizer nada de
/// `cifra_fio`: a exigencia e a de fabrica, que e o que se quer provar.
fn subir(base: &Path) -> (Arc<Servidor>, Portas) {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
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
              "usuarios": [
                {{ "id": 2, "login": "{LOGIN}", "nome": "Ana", "senha_hash": "{h}",
                   "ativo": true, "supervisor": true }} ],
              "web":  {{ "ligado": true, "bind": "127.0.0.1:0", "tls": true }},
              "rest": {{ "ligado": true, "bind": "127.0.0.1:0",
                         "swagger_ligado": true, "swagger_bind": "127.0.0.1:0",
                         "tls": true }}
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
    assert!(c.cifra_fio.exigir, "a prova e com a exigencia de fabrica");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let portas = Portas {
        web: comum::porta_real(|| s.porta_web()),
        rest: comum::porta_real(|| s.porta_rest()),
        swagger: comum::porta_real(|| s.porta_swagger()),
    };
    for porta in [portas.web, portas.rest, portas.swagger] {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let ate = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_err() {
            assert!(Instant::now() < ate, "a porta {porta} nao abriu em 5 s");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    (s, portas)
}

/// `curl` conferindo o certificado contra `ca`; devolve (codigo, corpo).
fn curl(
    ca: &Path,
    porta: u16,
    metodo: &str,
    caminho: &str,
    bearer: bool,
    corpo: &str,
) -> (u16, String) {
    let mut c = Command::new("curl");
    c.args(["-sS", "--max-time", "20", "--tlsv1.3", "-X", metodo])
        .arg("--cacert")
        .arg(ca)
        .args([
            "-H",
            "Content-Type: application/json",
            "-w",
            "\n%{http_code}",
        ]);
    if bearer {
        c.args(["-H", &format!("Authorization: Bearer {TOKEN}")]);
    }
    if !corpo.is_empty() {
        c.args(["--data-binary", corpo]);
    }
    let s = c
        .arg(format!("https://127.0.0.1:{porta}{caminho}"))
        .output()
        .unwrap();
    let saida = String::from_utf8_lossy(&s.stdout).into_owned();
    let (corpo, codigo) = saida.rsplit_once('\n').unwrap_or(("", ""));
    let codigo = codigo.trim().parse().unwrap_or_else(|_| {
        panic!(
            "curl sem codigo: {saida:?} {}",
            String::from_utf8_lossy(&s.stderr)
        )
    });
    (codigo, corpo.to_string())
}

#[test]
fn com_tls_as_portas_http_atendem_a_cifra_exigida_pelo_certificado_gerado() {
    let d = DirTemp::novo("tls-portas-http");
    let (_s, p) = subir(&d.0);
    let ca_web = d.0.join("tls-web-certificado.pem");
    let ca_rest = d.0.join("tls-rest-certificado.pem");
    assert!(ca_web.exists() && d.0.join("tls-web-chave.pem").exists());
    assert!(ca_rest.exists() && d.0.join("tls-rest-chave.pem").exists());

    let login =
        format!(r#"{{"token":"{TOKEN}","op":"login","usuario":"{LOGIN}","senha":"{SENHA}"}}"#);
    let (codigo, corpo) = curl(&ca_web, p.web, "POST", "/api", false, &login);
    assert_eq!(codigo, 200, "{corpo}");
    assert!(
        corpo.contains("sessao"),
        "o login nao abriu sessao: {corpo}"
    );

    let (codigo, corpo) = curl(&ca_rest, p.rest, "POST", "/v1/ping", true, "{}");
    assert_eq!(codigo, 200, "{corpo}");

    // O explorador e a SEGUNDA porta da secao rest, com o mesmo certificado.
    let (codigo, corpo) = curl(&ca_rest, p.swagger, "GET", "/", false, "");
    assert_eq!(codigo, 200, "{corpo}");
}

#[test]
fn a_porta_tls_nao_responde_em_claro() {
    let d = DirTemp::novo("tls-sem-claro");
    let (_s, p) = subir(&d.0);
    let mut c = TcpStream::connect(("127.0.0.1", p.web)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    c.write_all(b"GET /saude HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut resposta = Vec::new();
    let _ = c.read_to_end(&mut resposta);
    assert!(
        !String::from_utf8_lossy(&resposta).contains("HTTP/"),
        "a porta TLS respondeu HTTP em claro: {resposta:?}"
    );
    assert_eq!(
        resposta.first(),
        Some(&21),
        "esperava um alerta TLS: {resposta:?}"
    );
}

#[test]
fn o_autoassinado_sobrevive_a_nova_subida_e_a_chave_e_so_do_dono() {
    let d = DirTemp::novo("tls-reinicio");
    let (_s1, _) = subir(&d.0);
    let ca = d.0.join("tls-web-certificado.pem");
    let antes = std::fs::read(&ca).unwrap();

    // O segundo servidor do mesmo `config.json` e o reinicio: le o par que o
    // primeiro gravou em vez de gerar outro.
    let (_s2, p2) = subir(&d.0);
    assert_eq!(
        std::fs::read(&ca).unwrap(),
        antes,
        "o certificado mudou no reinicio"
    );
    // Quem confiou no certificado da primeira subida conversa com a segunda.
    let (codigo, corpo) = curl(&ca, p2.web, "GET", "/saude", false, "");
    assert_eq!(codigo, 200, "{corpo}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let modo = std::fs::metadata(d.0.join("tls-web-chave.pem"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            modo & 0o077,
            0,
            "a chave TLS e legivel por outros: {modo:o}"
        );
    }
}

#[test]
fn a_resposta_termina_com_close_notify() {
    // O `Drop` do fio manda o alerta: sem ele o cliente nao distingue a
    // resposta inteira de uma cortada no caminho (§6.1).
    let d = DirTemp::novo("tls-close-notify");
    let (_s, p) = subir(&d.0);
    let mut filho = Command::new("openssl")
        .args(["s_client", "-connect", &format!("127.0.0.1:{}", p.web)])
        .args(["-tls1_3", "-ign_eof", "-msg", "-CAfile"])
        .arg(d.0.join("tls-web-certificado.pem"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    filho
        .stdin
        .take()
        .unwrap()
        .write_all(b"GET /saude HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let s = filho.wait_with_output().unwrap();
    let saida = String::from_utf8_lossy(&s.stdout);
    assert!(saida.contains("200 OK"), "{saida}");
    assert!(
        saida
            .lines()
            .any(|l| l.starts_with("<<<") && l.contains("close_notify")),
        "o servidor fechou sem close_notify: {saida}"
    );
}
