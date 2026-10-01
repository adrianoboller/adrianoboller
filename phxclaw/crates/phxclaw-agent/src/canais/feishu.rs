//! Feishu (Lark): o evento `im.message.receive_v1` chega por webhook e a resposta sai pela
//! API de mensagens com o `tenant_access_token`.
//!
//! A entrada confere o `token` de verificacao que o proprio evento traz (o mecanismo do
//! modo sem cifra). O modo com Encrypt Key manda o corpo cifrado em AES-256-CBC, que o
//! agente nao decifra sem crate nova: evento cifrado e recusa com o motivo, nunca aceite
//! sem conferencia. O `tenant_access_token` e pedido a cada envio e nao fica guardado.

use super::caixa::{Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, erro_interno};
use super::cripto::iguais;
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://open.feishu.cn";

pub struct Feishu {
    caixa: Arc<Caixa>,
    verificacao: Credencial,
    segredo_app: Credencial,
    app_id: String,
    http: Http,
}

impl Feishu {
    pub fn novo(
        caixa: Arc<Caixa>,
        verificacao: Credencial,
        segredo_app: Credencial,
        app_id: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            verificacao,
            segredo_app,
            app_id,
            http: Http::novo(base, politica)?,
        })
    }

    /// O `tenant_access_token` a partir do segredo do app.
    fn login(&self, segredo: &str) -> Result<String, String> {
        let v = self.http.json(
            self.http
                .cliente()?
                .post(
                    self.http
                        .url("/open-apis/auth/v3/tenant_access_token/internal"),
                )
                .json(&json!({"app_id": self.app_id, "app_secret": segredo})),
        )?;
        if v["code"].as_i64() != Some(0) {
            return Err(format!("feishu recusou o login: {}", v["msg"]));
        }
        v["tenant_access_token"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "login sem tenant_access_token".into())
    }
}

/// O texto de uma mensagem `text` vem como JSON dentro de string: `{"text":"oi"}`.
pub fn mensagem_do_evento(v: &Value) -> Option<Mensagem> {
    if v["header"]["event_type"] != "im.message.receive_v1" {
        return None;
    }
    let e = &v["event"];
    let m = &e["message"];
    Some(Mensagem {
        conversa: m["chat_id"].as_str()?.to_string(),
        autor: e["sender"]["sender_id"]["open_id"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        id: m["message_id"].as_str().unwrap_or("").to_string(),
        texto: (m["message_type"] == "text")
            .then(|| {
                m["content"]
                    .as_str()
                    .and_then(|c| serde_json::from_str::<Value>(c).ok())
                    .and_then(|c| c["text"].as_str().map(|t| t.trim().to_string()))
            })
            .flatten(),
    })
}

impl Recebedor for Feishu {
    fn nome(&self) -> &str {
        "feishu"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
        if v.get("encrypt").is_some() {
            return Err((400, "evento cifrado (Encrypt Key) nao suportado".into()));
        }
        // O token mora no topo no aperto de mao e em `header` nos eventos v2.
        let dado = v["header"]["token"]
            .as_str()
            .or_else(|| v["token"].as_str())
            .unwrap_or("")
            .to_string();
        let ok = self
            .verificacao
            .com("verify", |s| {
                Ok(!dado.is_empty() && iguais(dado.as_bytes(), s.as_bytes()))
            })
            .map_err(erro_interno)?;
        if !ok {
            return Err((401, "token de verificacao nao confere".into()));
        }
        if v["type"] == "url_verification" {
            return Ok(RespostaWebhook::json(json!({"challenge": v["challenge"]})));
        }
        if let Some(m) = mensagem_do_evento(&v) {
            self.caixa
                .anexar(vec![(m, Value::Null)])
                .map_err(erro_interno)?;
        }
        Ok(RespostaWebhook::ok())
    }
}

impl Provedor for Feishu {
    fn nome(&self) -> &str {
        "feishu"
    }

    fn limite(&self) -> (usize, Unidade) {
        (30_000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let v = self.segredo_app.com_derivado(
            "send",
            |s| self.login(s),
            |token| {
                self.http.json(
                    self.http
                        .cliente()?
                        .post(self.http.url("/open-apis/im/v1/messages"))
                        .query(&[("receive_id_type", "chat_id")])
                        .bearer_auth(token)
                        .json(&json!({"receive_id": conversa, "msg_type": "text",
                            "content": json!({"text": texto}).to_string()})),
                )
            },
        )?;
        if v["code"].as_i64() != Some(0) {
            return Err(format!("feishu recusou: {}", v["msg"]));
        }
        Ok(v["data"]["message_id"].as_str().unwrap_or("").to_string())
    }
}
