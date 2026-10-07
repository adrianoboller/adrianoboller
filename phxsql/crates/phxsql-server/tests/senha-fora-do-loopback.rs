//! A senha em claro vinda de fora do loopback (pedido 667) -- provada PELO
//! SOQUETE, de um IP que nao e o 127.0.0.1.
//!
//! A tela aberta por `http://<IP da LAN>` nao tem `crypto.subtle`, cai na
//! reserva Base64 e manda `senha_b64`. O que cada prova segura:
//!
//! 1. o login com `senha_b64` vindo do IP da maquina (nao do loopback), com a
//!    web em claro e a exigencia de cifra desligada, e RECUSADO nomeando o
//!    escape -- e a recusa vem antes do cadastro: o usuario que nao existe
//!    recebe a MESMA;
//! 2. o login que vai para OUTRO servidor (`"servidor"` no pedido) nao passa
//!    pelo `op_login` daqui, e e recusado do mesmo jeito -- o caminho irmao;
//! 3. do loopback o mesmo pedido entra: a senha nao saiu da maquina;
//! 4. o `/saude` diz a tela que a reserva nao pode sair.

mod comum;
use comum::DirTemp;

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";
const SENHA: &str = "segredo123";

/// O IP desta maquina na rede, sem resolver nome: a rota que o sistema
/// escolheria para fora. `None` quando a maquina nao tem rede -- e a prova
/// diz que nao rodou em vez de passar calada.
fn ip_da_rede() -> Option<IpAddr> {
    let u = UdpSocket::bind("0.0.0.0:0").ok()?;
    u.connect("192.0.2.1:9").ok()?;
    let ip = u.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

fn subir(base: &Path, escape: bool) -> Arc<Servidor> {
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
              "cifra_fio": {{ "exigir": false, "senha_em_claro_pela_rede": {escape} }},
              "usuarios": [
                {{ "id": 2, "login": "ana", "nome": "Ana", "senha_hash": "{h}",
                   "ativo": true, "supervisor": true }} ],
              "web": {{ "ligado": true, "bind": "0.0.0.0:0" }}
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
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    s
}

/// Um pedido HTTP cru a `ip:porta`; devolve a resposta inteira.
fn http(ip: IpAddr, porta: u16, metodo: &str, rota: &str, corpo: &str) -> String {
    let mut c = TcpStream::connect((ip, porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    write!(
        c,
        "{metodo} {rota} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .unwrap();
    let mut v = Vec::new();
    let _ = c.read_to_end(&mut v);
    String::from_utf8_lossy(&v).into_owned()
}

fn b64(s: &str) -> String {
    phxsql_core::base64::codificar(s.as_bytes())
}

fn login(usuario: &str, extra: &str) -> String {
    format!(
        r#"{{"token":"{TOKEN}","op":"login","usuario_b64":"{}","senha_b64":"{}"{extra}}}"#,
        b64(usuario),
        b64(SENHA)
    )
}

#[test]
fn senha_em_claro_de_fora_do_loopback_se_recusa_pelos_dois_caminhos() {
    let Some(rede) = ip_da_rede() else {
        eprintln!("NAO RODOU: esta maquina nao tem IP de rede fora do loopback");
        return;
    };
    let d = DirTemp::novo("senha-667");
    let s = subir(&d.0, false);
    let porta = comum::porta_real(|| s.porta_web());

    // (4) a tela fica sabendo antes de mandar.
    let saude = http(rede, porta, "GET", "/saude", "");
    assert!(
        saude.contains("\"senha_em_claro_pela_rede\":false"),
        "{saude}"
    );

    // (1) o local, pelo IP da rede: recusado, nomeando o escape.
    let r = http(rede, porta, "POST", "/api", &login("ana", ""));
    assert!(r.contains("de fora deste computador, recusada"), "{r}");
    assert!(!r.contains("\"ok\":true"), "{r}");
    // ...e antes do cadastro: quem nao existe recebe a mesma recusa.
    let r = http(rede, porta, "POST", "/api", &login("ninguem", ""));
    assert!(r.contains("de fora deste computador, recusada"), "{r}");

    // (2) o irmao: o login que iria para OUTRO servidor. A porta 1 nao tem
    // ninguem -- se o portao nao viesse antes, o erro seria de conexao.
    let r = http(
        rede,
        porta,
        "POST",
        "/api",
        &login("ana", r#","servidor":"127.0.0.1:1""#),
    );
    assert!(r.contains("de fora deste computador, recusada"), "{r}");

    // (3) do loopback, o mesmo pedido entra.
    let r = http(
        "127.0.0.1".parse().unwrap(),
        porta,
        "POST",
        "/api",
        &login("ana", ""),
    );
    assert!(r.contains("\"ok\":true"), "{r}");
}

#[test]
fn com_o_escape_escrito_a_senha_de_fora_entra() {
    // O comportamento VELHO, por escolha escrita: quem aceita o risco
    // continua entrando pela LAN em claro.
    let Some(rede) = ip_da_rede() else {
        eprintln!("NAO RODOU: esta maquina nao tem IP de rede fora do loopback");
        return;
    };
    let d = DirTemp::novo("senha-667-escape");
    let s = subir(&d.0, true);
    let porta = comum::porta_real(|| s.porta_web());
    let saude = http(rede, porta, "GET", "/saude", "");
    assert!(
        saude.contains("\"senha_em_claro_pela_rede\":true"),
        "{saude}"
    );
    let r = http(rede, porta, "POST", "/api", &login("ana", ""));
    assert!(r.contains("\"ok\":true"), "{r}");
}
