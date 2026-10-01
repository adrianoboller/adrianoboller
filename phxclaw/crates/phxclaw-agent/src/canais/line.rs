//! LINE pela Messaging API: o evento chega por webhook assinado
//! (`X-Line-Signature: base64(HMAC-SHA256(channel secret, corpo))`) e a resposta sai por
//! `push`. Por `push` e nao por `reply` porque o `replyToken` vence em um minuto, e a
//! tarefa do agente pode demorar mais que isso.

use super::caixa::{Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, erro_interno};
use super::cripto::{base64, hmac_sha256, iguais};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://api.line.me";

pub struct Line {
    caixa: Arc<Caixa>,
    segredo_canal: Credencial,
    token: Credencial,
    http: Http,
}

impl Line {
    pub fn novo(
        caixa: Arc<Caixa>,
        segredo_canal: Credencial,
        token: Credencial,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            segredo_canal,
            token,
            http: Http::novo(base, politica)?,
        })
    }
}

/// Os eventos `message`. Em grupo a conversa e o grupo; em conversa direta, a pessoa.
pub fn mensagens_da_line(v: &Value) -> Vec<Mensagem> {
    v["events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["type"] == "message")
        .filter_map(|e| {
            let s = &e["source"];
            let conversa = s["groupId"]
                .as_str()
                .or_else(|| s["roomId"].as_str())
                .or_else(|| s["userId"].as_str())?
                .to_string();
            let m = &e["message"];
            Some(Mensagem {
                conversa,
                autor: s["userId"].as_str().unwrap_or("").to_string(),
                id: m["id"].as_str().unwrap_or("").to_string(),
                texto: (m["type"] == "text")
                    .then(|| m["text"].as_str().map(str::to_string))
                    .flatten(),
            })
        })
        .collect()
}

impl Recebedor for Line {
    fn nome(&self) -> &str {
        "line"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        let dada = p
            .cabecalho("x-line-signature")
            .ok_or((401, "falta X-Line-Signature".to_string()))?
            .trim()
            .to_string();
        let ok = self
            .segredo_canal
            .com("verify", |s| {
                let esperada = base64(&hmac_sha256(s.as_bytes(), &p.corpo));
                Ok(iguais(dada.as_bytes(), esperada.as_bytes()))
            })
            .map_err(erro_interno)?;
        if !ok {
            return Err((401, "assinatura nao confere".into()));
        }
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
        self.caixa
            .anexar(
                mensagens_da_line(&v)
                    .into_iter()
                    .map(|m| (m, Value::Null))
                    .collect(),
            )
            .map_err(erro_interno)?;
        Ok(RespostaWebhook::ok())
    }
}

impl Provedor for Line {
    fn nome(&self) -> &str {
        "line"
    }

    fn limite(&self) -> (usize, Unidade) {
        (5000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        // A chave de repeticao faz o reenvio do mesmo pedaco nao sair duas vezes.
        let chave = phxclaw_types::new_uuid_v7().to_string();
        self.token.com("send", |t| {
            let v = self.http.json(
                self.http
                    .cliente()?
                    .post(self.http.url("/v2/bot/message/push"))
                    .bearer_auth(t)
                    .header("X-Line-Retry-Key", &chave)
                    .json(&json!({"to": conversa, "messages": [{"type": "text", "text": texto}]})),
            )?;
            Ok(v["sentMessages"][0]["id"]
                .as_str()
                .unwrap_or(&chave)
                .to_string())
        })
    }
}
