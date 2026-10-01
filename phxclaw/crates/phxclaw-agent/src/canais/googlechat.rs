//! Google Chat: o evento chega por webhook do app (endpoint HTTP) e a resposta sai pelo
//! webhook de entrada do espaco.
//!
//! O que fica PARCIAL, e por que:
//! - o Google assina o evento com JWT RS256; conferir pede RSA, que o agente nao tem sem
//!   crate nova. A entrada confere uma chave na URL (`?chave=`), como no Teams;
//! - responder pela API (`spaces.messages.create`) exige conta de servico, que tambem
//!   assina JWT RS256. A saida usa o webhook de entrada do espaco -- a URL inteira e o
//!   segredo, guardado no broker --, e por isso so fala com o espaco desse webhook.

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, conferir_chave_na_url, erro_interno,
};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://chat.googleapis.com";

pub struct GoogleChat {
    caixa: Arc<Caixa>,
    chave_url: Credencial,
    /// A URL do webhook de entrada do espaco (com `key` e `token`), no broker.
    saida: Credencial,
    espaco: String,
    http: Http,
}

impl GoogleChat {
    pub fn novo(
        caixa: Arc<Caixa>,
        chave_url: Credencial,
        saida: Credencial,
        espaco: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            chave_url,
            saida,
            espaco,
            http: Http::novo(base, politica)?,
        })
    }
}

/// O evento MESSAGE. `argumentText` e o texto sem a mencao ao app, que e o pedido.
pub fn mensagem_do_evento(v: &Value) -> Option<Mensagem> {
    if v["type"] != "MESSAGE" {
        return None;
    }
    let m = &v["message"];
    Some(Mensagem {
        conversa: m["space"]["name"]
            .as_str()
            .or_else(|| v["space"]["name"].as_str())?
            .to_string(),
        autor: m["sender"]["name"].as_str().unwrap_or("").to_string(),
        id: m["name"].as_str().unwrap_or("").to_string(),
        texto: m["argumentText"]
            .as_str()
            .or_else(|| m["text"].as_str())
            .map(|t| t.trim().to_string()),
    })
}

impl Recebedor for GoogleChat {
    fn nome(&self) -> &str {
        "googlechat"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.chave_url
            .com("verify", |s| Ok(conferir_chave_na_url(p, s)))
            .map_err(erro_interno)??;
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
        if let Some(m) = mensagem_do_evento(&v) {
            self.caixa
                .anexar(vec![(m, Value::Null)])
                .map_err(erro_interno)?;
        }
        // Resposta vazia: a resposta de verdade vem depois, quando a tarefa termina.
        Ok(RespostaWebhook::ok())
    }
}

impl Provedor for GoogleChat {
    fn nome(&self) -> &str {
        "googlechat"
    }

    fn limite(&self) -> (usize, Unidade) {
        (4096, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        if conversa != self.espaco {
            return Err(format!(
                "o webhook de saida e do espaco {}; {conversa} nao tem saida",
                self.espaco
            ));
        }
        self.saida.com("send", |url| {
            self.http.conferir(url)?;
            let v = self
                .http
                .json(self.http.cliente()?.post(url).json(&json!({"text": texto})))?;
            Ok(v["name"].as_str().unwrap_or("").to_string())
        })
    }
}
