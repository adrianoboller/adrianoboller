//! O PhxZipWeb provado PELO SOQUETE (pedido 454, fatia Z5).
//!
//! Cada prova sobe a porta de verdade em `127.0.0.1:0`, conversa por
//! `TcpStream` e le a resposta como o navegador le: manda o pedido INTEIRO e
//! so depois le. Um cliente que lesse enquanto escreve veria o 413 mesmo sem
//! o dreno, e a prova do 413 passaria por engano.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use phxzip_web::{Config, Servidor, CSP};

/// Sobe a porta numa thread e devolve o endereco que o sistema deu.
fn subir(envio: usize) -> SocketAddr {
    let s = Servidor::escutar(Config { porta: 0, envio }).expect("a porta abre");
    let end = s.endereco().unwrap();
    std::thread::spawn(move || s.servir());
    end
}

/// Manda os bytes todos, e so entao le ate o fim.
fn conversar(end: SocketAddr, bruto: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut c = TcpStream::connect(end)?;
    c.set_read_timeout(Some(Duration::from_secs(20)))?;
    c.write_all(bruto)?;
    let mut resposta = Vec::new();
    c.read_to_end(&mut resposta)?;
    Ok(resposta)
}

fn get(end: SocketAddr, caminho: &str, host: &str) -> Vec<u8> {
    conversar(
        end,
        format!("GET {caminho} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes(),
    )
    .expect("resposta")
}

/// `(codigo, cabeca, corpo)`.
fn partir(resposta: &[u8]) -> (u16, String, Vec<u8>) {
    let fim = resposta
        .windows(4)
        .position(|j| j == b"\r\n\r\n")
        .expect("cabeca da resposta")
        + 4;
    let cabeca = String::from_utf8_lossy(&resposta[..fim]).into_owned();
    let codigo = cabeca[9..12].parse().unwrap();
    (codigo, cabeca, resposta[fim..].to_vec())
}

/// RED: com o `bind` em `0.0.0.0` (ou com uma opcao de endereco que alguem
/// ligasse), o endereco que o SISTEMA devolve deixa de ser o laco local.
#[test]
fn escuta_so_no_127_0_0_1() {
    let s = Servidor::escutar(Config {
        porta: 0,
        envio: 1024,
    })
    .unwrap();
    let end = s.endereco().unwrap();
    assert_eq!(end.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST), "{end}");
    assert_ne!(end.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
}

/// RED: sem a conferencia do `Host`, `evil.com` recebe a tela -- e e
/// exatamente o que um site religado para `127.0.0.1` pediria.
#[test]
fn host_alheio_e_recusado_pelo_soquete() {
    let end = subir(1024);
    let porta = end.port();
    for host in ["evil.com".to_string(), format!("evil.com:{porta}")] {
        let (codigo, cabeca, corpo) = partir(&get(end, "/", &host));
        assert_eq!(codigo, 403, "{host}");
        assert!(cabeca.contains(&format!("Content-Security-Policy: {CSP}\r\n")));
        assert_eq!(
            String::from_utf8(corpo).unwrap(),
            r#"{"ok":false,"erro":"HOST_RECUSADO"}"#
        );
    }
    // E o Host da casa recebe a tela inteira, byte a byte.
    let (codigo, cabeca, corpo) = partir(&get(end, "/", &format!("127.0.0.1:{porta}")));
    assert_eq!(codigo, 200);
    assert!(cabeca.contains("Content-Type: text/html; charset=utf-8\r\n"));
    assert_eq!(corpo, include_bytes!("../ui/index.html"));
}

/// RED: servindo a pasta `ui/` do disco (o atalho que o contrato proibe),
/// `/../Cargo.toml` devolve o manifesto deste crate.
#[test]
fn ponto_ponto_da_404_pelo_soquete() {
    let end = subir(1024);
    let host = format!("localhost:{}", end.port());
    for caminho in ["/../Cargo.toml", "/../src/lib.rs", "/%2e%2e/Cargo.toml"] {
        let (codigo, _, corpo) = partir(&get(end, caminho, &host));
        assert_eq!(codigo, 404, "{caminho}");
        assert_eq!(
            String::from_utf8(corpo).unwrap(),
            r#"{"ok":false,"erro":"ROTA_INEXISTENTE"}"#
        );
    }
}

/// A fonte e binaria e sai intacta: o caminho da resposta nao passa por texto.
#[test]
fn a_fonte_sai_byte_a_byte() {
    let end = subir(1024);
    let host = format!("127.0.0.1:{}", end.port());
    let (codigo, cabeca, corpo) = partir(&get(end, "/fonte/exo2-latin.woff2", &host));
    assert_eq!(codigo, 200);
    assert!(cabeca.contains("Content-Type: font/woff2\r\n"));
    assert_eq!(corpo, include_bytes!("../ui/fonte/exo2-latin.woff2"));
}

/// RED: sem o dreno depois do 413, o cliente que ainda esta escrevendo leva
/// `Connection reset` em vez da resposta -- no Chromium, `Failed to fetch`.
/// O corpo e 8 MiB contra um teto de 5 MiB: grande o bastante para o buffer
/// de recepcao nao engolir tudo sem ninguem lendo, e dentro do descarte
/// (o dobro do teto).
#[test]
fn corpo_acima_do_teto_devolve_413_legivel() {
    const LIMITE: usize = 5 * 1024 * 1024;
    const ENVIO: usize = 8 * 1024 * 1024;
    let end = subir(LIMITE);
    let mut bruto = format!(
        "POST /api/compactar HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {ENVIO}\r\n\r\n",
        end.port()
    )
    .into_bytes();
    bruto.extend(std::iter::repeat_n(0x5Au8, ENVIO));
    let resposta = conversar(end, &bruto).expect("o cliente le o 413, e nao leva RST");
    let (codigo, cabeca, corpo) = partir(&resposta);
    assert_eq!(codigo, 413);
    assert!(cabeca.starts_with("HTTP/1.1 413 Payload Too Large\r\n"));
    assert!(cabeca.contains("Content-Type: application/json; charset=utf-8\r\n"));
    assert_eq!(
        String::from_utf8(corpo).unwrap(),
        format!(
            r#"{{"ok":false,"erro":"GRANDE_DEMAIS","detalhe":{{"oque":"envio","declarado":{ENVIO},"teto":{LIMITE}}}}}"#
        )
    );
}
