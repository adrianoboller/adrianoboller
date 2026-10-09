//! O 413 CHEGA ao cliente -- pedido 454, fatia Z1, provado PELO SOQUETE.
//!
//! # O defeito que esta prova pega
//!
//! Recusar o `Content-Length` antes de ler o corpo e o certo (nao se le o que
//! nao se vai guardar). Mas fechar o soquete com o corpo ainda chegando deixa
//! bytes no buffer de recepcao, o TCP fecha com RST, e o cliente -- que ainda
//! esta ESCREVENDO -- leva `Connection reset` em vez do 413. No Chromium isso
//! e `Failed to fetch`, medido em 24/09/2026.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! RST e fato do sistema operacional, nao da funcao: um `&[u8]` em memoria
//! nao tem buffer de recepcao, e o teste passaria com ou sem o [`drenar`].
//!
//! # O cliente e o de navegador, de proposito
//!
//! Escreve o pedido INTEIRO e so depois le. Um cliente que lesse enquanto
//! escreve poderia ver o 413 mesmo sem dreno, e a prova passaria por engano.
//! O corpo tem 8 MiB: sem ninguem lendo do outro lado, o buffer de recepcao
//! nao cresce (o autotune do Linux so alarga quem esta lendo), e a escrita do
//! cliente fica presa ate o RST.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use phxsql_core::http::{drenar, ler_pedido, Excesso, PedidoLido, Tetos};

const TETOS: Tetos = Tetos {
    cabecalho: 16 * 1024,
    corpo: 64 * 1024,
};

/// O corpo que o cliente manda: 128 vezes o teto.
const ENVIO: usize = 8 * 1024 * 1024;

/// O teto de descarte do servidor de prova: o dobro do envio, como o contrato
/// do PhxZipWeb sugere (`docs/PHXZIP-WEB.md` §4).
const DESCARTE: u64 = 2 * ENVIO as u64;

/// Sobe um servidor de UM pedido que usa o motor do core, e devolve o
/// endereco e a thread.
fn servidor_de_um_pedido() -> (std::net::SocketAddr, thread::JoinHandle<Option<Excesso>>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = ouvinte.local_addr().unwrap();
    let t = thread::spawn(move || {
        let (mut s, _) = ouvinte.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        match ler_pedido(&mut s, TETOS) {
            PedidoLido::GrandeDemais(e) => {
                let corpo = format!("{{\"erro\":\"GRANDE_DEMAIS\",\"teto\":{}}}", TETOS.corpo);
                let resposta = format!(
                    "HTTP/1.1 {} Payload Too Large\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
                    e.codigo_http(),
                    corpo.len()
                );
                s.write_all(resposta.as_bytes()).unwrap();
                s.flush().unwrap();
                drenar(&mut s, DESCARTE, Duration::from_millis(250));
                Some(e)
            }
            PedidoLido::Pedido(p) => {
                // Devolve o corpo de volta, para o cliente conferir o byte.
                let mut r = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    p.corpo.len()
                )
                .into_bytes();
                r.extend_from_slice(&p.corpo);
                s.write_all(&r).unwrap();
                None
            }
            PedidoLido::Nada => None,
        }
    });
    (end, t)
}

/// Manda o pedido inteiro e so entao le a resposta -- como um navegador.
fn mandar(end: std::net::SocketAddr, corpo: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut c = TcpStream::connect(end)?;
    c.set_read_timeout(Some(Duration::from_secs(10)))?;
    c.set_write_timeout(Some(Duration::from_secs(10)))?;
    let cabeca = format!(
        "POST /api/compactar HTTP/1.1\r\nHost: 127.0.0.1\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
        corpo.len()
    );
    c.write_all(cabeca.as_bytes())?;
    c.write_all(corpo)?;
    let mut resposta = Vec::new();
    c.read_to_end(&mut resposta)?;
    Ok(resposta)
}

/// RED: tire o `drenar` do servidor acima e o cliente leva `Connection reset`
/// (ou `Broken pipe`) na escrita, sem nunca ver o 413.
#[test]
fn o_cliente_le_o_413_pelo_soquete() {
    let (end, servidor) = servidor_de_um_pedido();
    let resposta = mandar(end, &vec![0x5Au8; ENVIO]);
    let excesso = servidor.join().unwrap();
    assert_eq!(
        excesso,
        Some(Excesso::Corpo {
            declarado: ENVIO as u64,
            teto: TETOS.corpo
        })
    );
    let resposta = resposta.expect("o cliente tem de ler a resposta, e nao levar RST");
    let texto = String::from_utf8_lossy(&resposta);
    assert!(texto.starts_with("HTTP/1.1 413 "), "{texto}");
    assert!(texto.contains("GRANDE_DEMAIS"), "{texto}");
}

/// RED: com o corpo passando por `String::from_utf8_lossy`, como o leitor do
/// servidor fazia antes, o 0xFF volta como `EF BF BD` e o tamanho muda.
#[test]
fn o_byte_0xff_atravessa_o_soquete_intacto() {
    let (end, servidor) = servidor_de_um_pedido();
    let corpo: Vec<u8> = (0..=255u8).rev().cycle().take(4096).collect();
    let resposta = mandar(end, &corpo).unwrap();
    assert_eq!(servidor.join().unwrap(), None);
    let fim = resposta
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("cabecalho da resposta")
        + 4;
    assert!(resposta.starts_with(b"HTTP/1.1 200 "));
    assert_eq!(&resposta[fim..], &corpo[..]);
    assert_eq!(resposta[fim], 0xFF);
}
