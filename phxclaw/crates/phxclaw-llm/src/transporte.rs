//! O unico caminho de rede dos quatro provedores.
//!
//! Um laco so pelo mesmo motivo do `seguranca.rs`: timeout, teto de resposta, redirecionamento
//! desligado e erro sem URL sao decisoes que, repetidas por provedor, divergem no dia em que
//! alguem mexe em uma so.

use crate::seguranca::{ApiKey, Base};
use phxclaw_agent_core::LlmError;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::time::Duration;

/// Resposta maior que isto nao e resposta de chat: e defeito ou abuso, e ler tudo para a
/// memoria antes de recusar seria o proprio estrago.
pub(crate) const TETO_RESPOSTA: usize = 16 * 1024 * 1024;
/// Corpo de erro vai para mensagem; mais que isso so polui o log.
const TETO_CORPO_ERRO: usize = 2048;

pub(crate) const TIMEOUT_NUVEM: Duration = Duration::from_secs(120);
/// Modelo local em CPU e lento no primeiro token; 120 s ja cortou geracao legitima.
pub(crate) const TIMEOUT_LOCAL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub(crate) struct Transporte {
    cliente: reqwest::Client,
}

impl Transporte {
    pub fn novo(base: &Base, timeout: Duration) -> Result<Self, LlmError> {
        let mut b = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(15))
            // Redirecionamento levaria o cabecalho da chave para uma origem que a politica
            // nunca conferiu.
            .redirect(reqwest::redirect::Policy::none());
        if base.loopback {
            // Loopback pelo proxy do sistema sairia da maquina (ou falharia no proxy).
            b = b.no_proxy();
        }
        let cliente = b
            .build()
            .map_err(|e| LlmError::Transport(texto_do_erro(e, None)))?;
        Ok(Self { cliente })
    }

    /// POST de JSON; devolve o JSON da resposta 2xx ou o erro ja redigido.
    pub async fn post_json(
        &self,
        url: &str,
        cabecalhos: &[(&'static str, &str)],
        chave: Option<&ApiKey>,
        corpo: &Value,
    ) -> Result<Value, LlmError> {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        for (nome, valor) in cabecalhos {
            let mut v = HeaderValue::from_str(valor)
                .map_err(|_| LlmError::Credential(format!("cabecalho {nome} invalido")))?;
            // Marca como sensivel todo cabecalho que carrega a chave: o Debug do reqwest e
            // do hyper passa a mostra-lo como "Sensitive".
            if chave.is_some_and(|k| valor.contains(k.expor())) {
                v.set_sensitive(true);
            }
            h.insert(HeaderName::from_static(nome), v);
        }
        let bytes = serde_json::to_vec(corpo).map_err(|e| LlmError::Parse(e.to_string()))?;
        let mut resp = self
            .cliente
            .post(url)
            .headers(h)
            .body(bytes)
            .send()
            .await
            .map_err(|e| LlmError::Transport(texto_do_erro(e, chave)))?;
        let status = resp.status();
        let mut corpo = Vec::new();
        while let Some(pedaco) = resp
            .chunk()
            .await
            .map_err(|e| LlmError::Transport(texto_do_erro(e, chave)))?
        {
            if corpo.len() + pedaco.len() > TETO_RESPOSTA {
                return Err(LlmError::Denied(format!(
                    "resposta acima do teto de {TETO_RESPOSTA} bytes"
                )));
            }
            corpo.extend_from_slice(&pedaco);
        }
        if !status.is_success() {
            let texto = String::from_utf8_lossy(&corpo);
            let texto = cortar(&texto, TETO_CORPO_ERRO);
            return Err(LlmError::Api {
                status: status.as_u16(),
                body: chave.map_or(texto.clone(), |k| k.tapar(&texto)),
            });
        }
        serde_json::from_slice(&corpo).map_err(|e| {
            // O corpo nao vai para a mensagem: pode ser grande e pode ecoar o pedido.
            LlmError::Parse(format!("JSON invalido ({} bytes): {e}", corpo.len()))
        })
    }
}

fn cortar(t: &str, max: usize) -> String {
    if t.len() <= max {
        return t.to_owned();
    }
    let mut fim = max;
    while !t.is_char_boundary(fim) {
        fim -= 1;
    }
    format!("{}...[cortado]", &t[..fim])
}

/// `without_url` sempre: medido nesta base, a URL do erro do reqwest ja carregou token.
/// A cadeia de causas entra porque "error sending request" sozinho nao diagnostica nada.
fn texto_do_erro(e: reqwest::Error, chave: Option<&ApiKey>) -> String {
    let timeout = e.is_timeout();
    let e = e.without_url();
    let mut t = e.to_string();
    let mut fonte = std::error::Error::source(&e);
    while let Some(s) = fonte {
        t.push_str(": ");
        t.push_str(&s.to_string());
        fonte = s.source();
    }
    if timeout {
        t.push_str(" (timeout)");
    }
    chave.map_or(t.clone(), |k| k.tapar(&t))
}
