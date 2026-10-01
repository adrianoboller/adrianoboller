//! Viber pela REST API de bots: o evento chega por webhook assinado
//! (`X-Viber-Content-Signature: hex(HMAC-SHA256(auth token, corpo))`) e a resposta sai por
//! `send_message`. O mesmo auth token assina e autentica -- uma credencial so no broker.
//! O Viber responde 200 mesmo quando recusa; o erro de verdade e `status` diferente de 0.

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, conferir_hmac_hex, erro_interno,
};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://chatapi.viber.com";

pub struct Viber {
    caixa: Arc<Caixa>,
    token: Credencial,
    remetente: String,
    http: Http,
}

impl Viber {
    /// `remetente` e o nome que aparece na mensagem (o Viber exige).
    pub fn novo(
        caixa: Arc<Caixa>,
        token: Credencial,
        remetente: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            token,
            remetente,
            http: Http::novo(base, politica)?,
        })
    }
}

pub fn mensagem_do_viber(v: &Value) -> Option<Mensagem> {
    if v["event"] != "message" {
        return None;
    }
    let de = v["sender"]["id"].as_str()?.to_string();
    let m = &v["message"];
    Some(Mensagem {
        conversa: de.clone(),
        autor: de,
        id: v["message_token"]
            .as_u64()
            .map(|t| t.to_string())
            .or_else(|| v["message_token"].as_str().map(str::to_string))
            .unwrap_or_default(),
        texto: (m["type"] == "text")
            .then(|| m["text"].as_str().map(str::to_string))
            .flatten(),
    })
}

impl Recebedor for Viber {
    fn nome(&self) -> &str {
        "viber"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.token
            .com("verify", |s| {
                Ok(conferir_hmac_hex(p, "x-viber-content-signature", "", s))
            })
            .map_err(erro_interno)??;
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
        if let Some(m) = mensagem_do_viber(&v) {
            self.caixa
                .anexar(vec![(m, Value::Null)])
                .map_err(erro_interno)?;
        }
        Ok(RespostaWebhook::ok())
    }
}

impl Provedor for Viber {
    fn nome(&self) -> &str {
        "viber"
    }

    fn limite(&self) -> (usize, Unidade) {
        (7000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        self.token.com("send", |t| {
            let v = self.http.json(
                self.http
                    .cliente()?
                    .post(self.http.url("/pa/send_message"))
                    .header("X-Viber-Auth-Token", t)
                    .json(&json!({"receiver": conversa, "type": "text", "text": texto,
                        "sender": {"name": self.remetente}})),
            )?;
            if v["status"].as_i64() != Some(0) {
                return Err(format!(
                    "viber recusou: {}",
                    v["status_message"].as_str().unwrap_or("sem detalhe")
                ));
            }
            Ok(v["message_token"].to_string())
        })
    }
}
