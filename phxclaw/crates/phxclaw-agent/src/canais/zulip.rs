//! Zulip pela REST API: conversas diretas com o bot, lidas por `GET /messages` com o
//! `anchor` no ultimo id visto (o cursor), e respondidas por `POST /messages`.
//!
//! So conversa direta: em stream, todo mundo do stream falaria com o agente, e a lista de
//! permitidos e de pessoas (o e-mail de quem escreve). `is:private` e o nome antigo de
//! `is:dm`, aceito pelos servidores velhos e pelos novos.

use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade, pausa_se_vazio};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use reqwest::blocking::RequestBuilder;
use serde_json::{Value, json};

pub struct Zulip {
    http: Http,
    cred: Credencial,
    email: String,
}

impl Zulip {
    /// `email` e o do bot: usuario do Basic e o remetente que se ignora.
    pub fn novo(
        cred: Credencial,
        email: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(base, politica)?,
            cred,
            email,
        })
    }

    fn pedir(
        &self,
        uso: &str,
        rb: impl Fn() -> Result<RequestBuilder, String>,
    ) -> Result<Value, String> {
        self.cred.com(uso, |chave| {
            let v = self.http.json(rb()?.basic_auth(&self.email, Some(chave)))?;
            if v["result"] != "success" {
                return Err(format!(
                    "zulip recusou: {}",
                    v["msg"].as_str().unwrap_or("sem detalhe")
                ));
            }
            Ok(v)
        })
    }
}

impl Provedor for Zulip {
    fn nome(&self) -> &str {
        "zulip"
    }

    /// O Zulip recusa mensagem acima de 10000 bytes.
    fn limite(&self) -> (usize, Unidade) {
        (10_000, Unidade::Byte)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let narrow = json!([{"operator": "is", "operand": "private"}]).to_string();
        let url = self.http.url("/api/v1/messages");
        let v = self.pedir("receive", || {
            let mut q = vec![
                ("narrow", narrow.clone()),
                ("apply_markdown", "false".into()),
            ];
            match cursor {
                Some(c) => q.extend([
                    ("anchor", c.to_string()),
                    ("include_anchor", "false".into()),
                    ("num_before", "0".into()),
                    ("num_after", "100".into()),
                ]),
                None => q.extend([
                    ("anchor", "newest".into()),
                    ("num_before", "1".into()),
                    ("num_after", "0".into()),
                ]),
            }
            Ok(self.http.cliente()?.get(&url).query(&q))
        })?;
        let mut msgs = v["messages"].as_array().cloned().unwrap_or_default();
        msgs.sort_by_key(|m| m["id"].as_u64().unwrap_or(0));
        let mut lote = Vec::new();
        if cursor.is_none() {
            let ultimo = msgs.last().and_then(|m| m["id"].as_u64()).unwrap_or(0);
            lote.push(Entrada {
                cursor: Some(ultimo.to_string()),
                mensagem: None,
            });
            return Ok(lote);
        }
        for m in msgs {
            let Some(id) = m["id"].as_u64() else { continue };
            let de = m["sender_email"].as_str().unwrap_or("").to_string();
            let alheia = !de.eq_ignore_ascii_case(&self.email);
            lote.push(Entrada {
                cursor: Some(id.to_string()),
                mensagem: alheia.then(|| Mensagem {
                    conversa: de.clone(),
                    autor: de,
                    id: id.to_string(),
                    texto: m["content"].as_str().map(str::to_string),
                }),
            });
        }
        pausa_se_vazio(&lote, espera_seg);
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let para = json!([conversa]).to_string();
        let v = self.pedir("send", || {
            Ok(self
                .http
                .cliente()?
                .post(self.http.url("/api/v1/messages"))
                .form(&[
                    ("type", "private"),
                    ("to", para.as_str()),
                    ("content", texto),
                ]))
        })?;
        Ok(v["id"].as_u64().map(|i| i.to_string()).unwrap_or_default())
    }
}
