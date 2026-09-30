//! Servidor HTTP falso sobre o `TcpListener` da std: captura UM pedido e devolve uma
//! resposta fixa. Soquete de verdade de proposito -- a prova passa pelo mesmo reqwest,
//! cabecalhos e corpo que iriam para o provedor.

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

#[derive(Debug)]
pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    /// Nomes em minusculas.
    pub cabecalhos: Vec<(String, String)>,
    pub corpo: Value,
}

impl Pedido {
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, v)| v.as_str())
    }
}

/// Sobe o servidor; devolve a base (`http://127.0.0.1:PORTA`) e o receptor do pedido.
pub fn subir(status: u16, resposta: String) -> (String, mpsc::Receiver<Pedido>) {
    subir_varios(vec![(status, resposta)])
}

/// Uma conexao por resposta, na ordem dada (a resposta leva `connection: close`).
pub fn subir_varios(respostas: Vec<(u16, String)>) -> (String, mpsc::Receiver<Pedido>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", ouvinte.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for (status, resposta) in respostas {
            let (soquete, _) = ouvinte.accept().unwrap();
            let mut leitor = BufReader::new(soquete.try_clone().unwrap());
            let mut linha = String::new();
            leitor.read_line(&mut linha).unwrap();
            let mut partes = linha.split_whitespace();
            let metodo = partes.next().unwrap_or_default().to_owned();
            let caminho = partes.next().unwrap_or_default().to_owned();
            let mut cabecalhos = Vec::new();
            let mut tamanho = 0usize;
            loop {
                let mut l = String::new();
                leitor.read_line(&mut l).unwrap();
                let l = l.trim_end();
                if l.is_empty() {
                    break;
                }
                let (n, v) = l.split_once(':').unwrap();
                let n = n.trim().to_ascii_lowercase();
                let v = v.trim().to_owned();
                if n == "content-length" {
                    tamanho = v.parse().unwrap();
                }
                cabecalhos.push((n, v));
            }
            let mut corpo = vec![0u8; tamanho];
            leitor.read_exact(&mut corpo).unwrap();
            let corpo = serde_json::from_slice(&corpo).unwrap_or(Value::Null);
            let mut s = soquete;
            write!(
            s,
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{resposta}",
            resposta.len()
        )
        .unwrap();
            s.flush().unwrap();
            let _ = tx.send(Pedido {
                metodo,
                caminho,
                cabecalhos,
                corpo,
            });
        }
    });
    (base, rx)
}
