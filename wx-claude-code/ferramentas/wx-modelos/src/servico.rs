//! Conversa com o servico de modelo local (Magnitude, em 127.0.0.1:10100).
//!
//! HTTP/1.1 na mao sobre TcpStream: o suficiente para GET e POST de JSON, com
//! tempo limite em TODAS as pontas. Sem tempo limite, servico pendurado vira
//! terminal travado sem explicacao -- e o usuario culpa o plugin.
//!
//! O que este modulo NAO faz: nao sobe o servico, nao baixa modelo e nao
//! redistribui nada do Magnitude. Ele controla o que ja esta instalado.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::json::{analisar, Valor};

pub const ENDERECO_PADRAO: &str = "127.0.0.1:10100";
const ESPERA: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum Falha {
    SemServico(String),
    Http(u16, String),
    Corpo(String),
}

impl std::fmt::Display for Falha {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Falha::SemServico(e) => {
                write!(f, "servico local fora do ar em {ENDERECO_PADRAO} ({e})")
            }
            Falha::Http(c, e) => write!(f, "servico respondeu {c}: {e}"),
            Falha::Corpo(e) => write!(f, "resposta do servico ilegivel: {e}"),
        }
    }
}

fn abrir(endereco: &str) -> Result<TcpStream, Falha> {
    let alvo = endereco
        .to_socket_addrs()
        .map_err(|e| Falha::SemServico(e.to_string()))?
        .next()
        .ok_or_else(|| Falha::SemServico("endereco sem resolucao".into()))?;
    let fluxo =
        TcpStream::connect_timeout(&alvo, ESPERA).map_err(|e| Falha::SemServico(e.to_string()))?;
    fluxo.set_read_timeout(Some(ESPERA)).ok();
    fluxo.set_write_timeout(Some(ESPERA)).ok();
    Ok(fluxo)
}

fn pedir(
    endereco: &str,
    metodo: &str,
    caminho: &str,
    corpo: Option<&str>,
) -> Result<String, Falha> {
    let mut fluxo = abrir(endereco)?;
    let mut req = format!(
        "{metodo} {caminho} HTTP/1.1\r\nHost: {endereco}\r\nConnection: close\r\nAccept: application/json\r\n"
    );
    if let Some(c) = corpo {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            c.len()
        ));
    }
    req.push_str("\r\n");
    if let Some(c) = corpo {
        req.push_str(c);
    }
    fluxo
        .write_all(req.as_bytes())
        .map_err(|e| Falha::SemServico(e.to_string()))?;
    let mut bruto = Vec::new();
    fluxo
        .read_to_end(&mut bruto)
        .map_err(|e| Falha::SemServico(e.to_string()))?;
    let texto = String::from_utf8_lossy(&bruto).to_string();
    let (cabecalho, corpo_resp) = texto
        .split_once("\r\n\r\n")
        .ok_or_else(|| Falha::Corpo("resposta sem corpo".into()))?;
    let codigo: u16 = cabecalho
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| Falha::Corpo("resposta sem codigo".into()))?;
    // corpo pode vir em chunks: junta os pedacos antes de analisar
    let corpo_resp = if cabecalho
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        desfragmentar(corpo_resp)
    } else {
        corpo_resp.to_string()
    };
    if !(200..300).contains(&codigo) {
        return Err(Falha::Http(codigo, corpo_resp.chars().take(200).collect()));
    }
    Ok(corpo_resp)
}

fn desfragmentar(corpo: &str) -> String {
    let mut saida = String::new();
    let mut resto = corpo;
    while let Some((tam, r)) = resto.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(tam.trim().split(';').next().unwrap_or("0"), 16) else {
            break;
        };
        if n == 0 || r.len() < n {
            break;
        }
        saida.push_str(&r[..n]);
        resto = r[n..].strip_prefix("\r\n").unwrap_or("");
    }
    saida
}

/// Servico no ar? Uma pergunta, com tempo limite, sem efeito colateral.
pub fn no_ar(endereco: &str) -> bool {
    abrir(endereco).is_ok()
}

pub fn json(endereco: &str, caminho: &str) -> Result<Valor, Falha> {
    let corpo = pedir(endereco, "GET", caminho, None)?;
    analisar(&corpo).map_err(|e| Falha::Corpo(e.to_string()))
}

pub fn json_post(endereco: &str, caminho: &str, corpo: &str) -> Result<Valor, Falha> {
    let resposta = pedir(endereco, "POST", caminho, Some(corpo))?;
    if resposta.trim().is_empty() {
        return Ok(Valor::Nulo);
    }
    analisar(&resposta).map_err(|e| Falha::Corpo(e.to_string()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn desfragmenta_corpo_em_pedacos() {
        assert_eq!(
            desfragmentar("4\r\nabcd\r\n3\r\nefg\r\n0\r\n\r\n"),
            "abcdefg"
        );
    }

    #[test]
    fn servico_fechado_nao_trava_nem_mente() {
        // porta improvavel: tem de dizer que nao ha servico, e rapido
        let t = std::time::Instant::now();
        assert!(!no_ar("127.0.0.1:1"));
        assert!(t.elapsed() < ESPERA * 2, "demorou demais para desistir");
    }
}
