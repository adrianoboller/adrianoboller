//! A porta do PhxZip web provada pelo SOQUETE, com o servidor de pe: o que
//! depende de rede se prova contra a rede, nao chamando funcao por dentro.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use phxzip::{Arquivo7z, Escritor, Limites, Opcoes};
use phxzip_web::{servir_em, Config};

const USUARIO: &str = "adriano";
const SENHA: &str = "senha-do-teste-web";

fn subir(com_login: bool, max_corpo: usize) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let mut c = Config {
        max_corpo,
        ..Config::default()
    };
    if com_login {
        c.usuario = Some(USUARIO.into());
        // Poucas voltas so para o teste nao pagar o PBKDF2 inteiro.
        c.senha_hash = Some(phxsql_core::senha::cifrar_com(SENHA, 1000));
    }
    std::thread::spawn(move || servir_em(ouvinte, c));
    porta
}

struct Resposta {
    codigo: u16,
    cabecalhos: String,
    corpo: Vec<u8>,
}

impl Resposta {
    fn texto(&self) -> String {
        String::from_utf8_lossy(&self.corpo).into_owned()
    }
    fn cookie(&self) -> Option<String> {
        self.cabecalhos.lines().find_map(|l| {
            let v = l.strip_prefix("Set-Cookie: ")?;
            Some(v.split(';').next()?.to_string())
        })
    }
}

fn pedir(porta: u16, metodo: &str, rota: &str, extras: &[(&str, &str)], corpo: &[u8]) -> Resposta {
    let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let mut cab = format!(
        "{metodo} {rota} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n",
        corpo.len()
    );
    for (k, v) in extras {
        cab.push_str(&format!("{k}: {v}\r\n"));
    }
    cab.push_str("\r\n");
    s.write_all(cab.as_bytes()).unwrap();
    let _ = s.write_all(corpo);
    let mut tudo = Vec::new();
    let _ = s.read_to_end(&mut tudo);
    let fim = tudo
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("resposta sem cabecalho");
    let cabecalhos = String::from_utf8_lossy(&tudo[..fim]).into_owned();
    let codigo = cabecalhos
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    Resposta {
        codigo,
        cabecalhos,
        corpo: tudo[fim + 4..].to_vec(),
    }
}

fn envelope(meta: &str, bytes: &[u8]) -> Vec<u8> {
    let mut v = meta.as_bytes().to_vec();
    v.push(b'\n');
    v.extend_from_slice(bytes);
    v
}

fn api(porta: u16, rota: &str, cookie: Option<&str>, corpo: &[u8]) -> Resposta {
    let mut h = vec![("X-PhxZip", "1")];
    let c;
    if let Some(k) = cookie {
        c = k.to_string();
        h.push(("Cookie", c.as_str()));
    }
    pedir(porta, "POST", rota, &h, corpo)
}

fn um_7z(senha: Option<&str>) -> Vec<u8> {
    let mut e = Escritor::novo(Opcoes {
        senha: senha.map(String::from),
        acaso: [3; 32],
        ..Opcoes::default()
    });
    e.arquivo(
        "docs/leia.txt",
        b"PhxZip na porta 4000\n".repeat(50),
        None,
        None,
    )
    .unwrap();
    e.arquivo("b.bin", (0..=255u8).collect(), None, None)
        .unwrap();
    e.gravar().unwrap()
}

#[test]
fn sem_login_compacta_lista_testa_e_extrai() {
    let p = subir(false, 1 << 20);
    let r = pedir(p, "GET", "/api/estado", &[], b"");
    assert!(r.texto().contains("\"exige_login\":false"), "{}", r.texto());
    let pagina = pedir(p, "GET", "/", &[], b"");
    assert_eq!(pagina.codigo, 200);
    assert!(pagina.texto().contains("PhxZip"));
    assert!(pagina.cabecalhos.contains("frame-ancestors 'none'"));

    let dados = b"conteudo que vai e volta pela porta\n".repeat(30);
    let meta = format!(
        r#"{{"nivel":9,"senha":"s3gr3do","arquivos":[{{"nome":"pasta/a.txt","tamanho":{}}}]}}"#,
        dados.len()
    );
    let r = api(p, "/api/compactar", None, &envelope(&meta, &dados));
    assert_eq!(r.codigo, 200, "{}", r.texto());
    assert!(r.cabecalhos.contains("application/x-7z-compressed"));
    // O que a porta devolveu e 7z de verdade, com nomes cifrados.
    let arq = r.corpo;
    let a = Arquivo7z::abrir(&arq, Some("s3gr3do"), Limites::default()).unwrap();
    assert!(a.cabecalho_cifrado());
    assert_eq!(a.extrair(0).unwrap(), dados);
    drop(a);
    let r = api(
        p,
        "/api/listar",
        None,
        &envelope(r#"{"senha":"s3gr3do"}"#, &arq),
    );
    assert!(r.texto().contains("pasta/a.txt"), "{}", r.texto());
    let r = api(
        p,
        "/api/testar",
        None,
        &envelope(r#"{"senha":"s3gr3do"}"#, &arq),
    );
    assert!(r.texto().contains("\"testado\":true"), "{}", r.texto());
    let r = api(
        p,
        "/api/extrair",
        None,
        &envelope(r#"{"senha":"s3gr3do","indice":0}"#, &arq),
    );
    assert_eq!(r.corpo, dados);
    let r = api(
        p,
        "/api/listar",
        None,
        &envelope(r#"{"senha":"errada"}"#, &arq),
    );
    assert_eq!(r.codigo, 400);
    assert!(r.texto().contains("senha_errada"));
}

#[test]
fn com_login_nada_de_arquivo_sem_sessao() {
    let p = subir(true, 1 << 20);
    let arq = um_7z(None);
    let corpo = envelope("{}", &arq);
    assert!(pedir(p, "GET", "/api/estado", &[], b"")
        .texto()
        .contains("\"exige_login\":true"));
    assert_eq!(api(p, "/api/listar", None, &corpo).codigo, 401);

    let errada = format!(r#"{{"usuario":"{USUARIO}","senha":"nao-e"}}"#);
    let r = api(p, "/api/entrar", None, errada.as_bytes());
    assert_eq!(r.codigo, 401);
    assert!(r.cookie().is_none());

    let certa = format!(r#"{{"usuario":"{USUARIO}","senha":"{SENHA}"}}"#);
    let r = api(p, "/api/entrar", None, certa.as_bytes());
    assert_eq!(r.codigo, 200, "{}", r.texto());
    let cookie = r.cookie().expect("sessao");
    assert!(r.cabecalhos.contains("HttpOnly") && r.cabecalhos.contains("SameSite=Strict"));
    assert!(!r.texto().contains(SENHA), "a senha voltou na resposta");

    let r = api(p, "/api/listar", Some(&cookie), &corpo);
    assert_eq!(r.codigo, 200, "{}", r.texto());
    assert!(r.texto().contains("docs/leia.txt"));
    let e = pedir(p, "GET", "/api/estado", &[("Cookie", &cookie)], b"");
    assert!(e.texto().contains("\"logado\":true"));
    assert!(!e.texto().contains("pbkdf2") && !e.texto().contains(SENHA));

    api(p, "/api/sair", Some(&cookie), b"");
    assert_eq!(api(p, "/api/listar", Some(&cookie), &corpo).codigo, 401);
}

#[test]
fn sem_o_cabecalho_da_casa_e_recusado_mesmo_sem_login() {
    let p = subir(false, 1 << 20);
    let r = pedir(p, "POST", "/api/listar", &[], &envelope("{}", &um_7z(None)));
    assert_eq!(r.codigo, 403);
}

#[test]
fn corpo_acima_do_teto_e_413_antes_de_ler() {
    let p = subir(false, 4096);
    let r = api(p, "/api/listar", None, &envelope("{}", &vec![0u8; 20_000]));
    assert_eq!(r.codigo, 413, "{}", r.texto());
}

#[test]
fn nome_que_sai_da_pasta_nao_entra_no_arquivo() {
    let p = subir(false, 1 << 20);
    let meta = r#"{"arquivos":[{"nome":"../../.bashrc","tamanho":3}]}"#;
    let r = api(p, "/api/compactar", None, &envelope(meta, b"abc"));
    assert_eq!(r.codigo, 400);
    assert!(r.texto().contains("\"tipo\":\"caminho\""), "{}", r.texto());
}
