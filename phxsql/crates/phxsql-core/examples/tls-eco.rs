//! Servidor HTTPS minimo sobre o TLS 1.3 desta casa, para provar o aperto
//! contra um navegador de verdade (pedido 572, T4).
//!
//! `cargo run --example tls-eco -p phxsql-core -- PORTA [CONEXOES]` atende
//! CONEXOES conexoes (padrao 4) com uma pagina que diz o grupo e o ALPN
//! negociados, e sai.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use phxsql_core::tls::{aceitar, Identidade};

fn main() {
    let mut args = std::env::args().skip(1);
    let porta: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(8443);
    let conexoes: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(4);
    let id = Identidade::autoassinada(&["localhost", "127.0.0.1"]).expect("certificado");
    let ouvinte = TcpListener::bind(("127.0.0.1", porta)).expect("porta");
    println!("ouvindo em https://127.0.0.1:{porta}/");
    for fio in ouvinte.incoming().take(conexoes) {
        let Ok(fio) = fio else { continue };
        let _ = fio.set_read_timeout(Some(Duration::from_secs(10)));
        let mut t = match aceitar(fio, &id, &[b"http/1.1"]) {
            Ok(t) => t,
            Err(e) => {
                println!("aperto recusado: {e}");
                continue;
            }
        };
        let mut pedido = Vec::new();
        let mut b = [0u8; 1024];
        while !pedido.windows(4).any(|w| w == b"\r\n\r\n") {
            match t.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => pedido.extend_from_slice(&b[..n]),
            }
        }
        let n = t.negociado();
        let corpo = format!(
            "<!doctype html><title>tls-eco</title><p id=r>grupo=0x{:04x} alpn={}</p>",
            n.grupo,
            n.alpn
                .as_deref()
                .map(|a| String::from_utf8_lossy(a).into_owned())
                .unwrap_or_else(|| "-".into())
        );
        let _ = write!(
            t,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
            corpo.len()
        );
        let _ = t.despedir();
        println!("atendido: {}", corpo);
    }
}
