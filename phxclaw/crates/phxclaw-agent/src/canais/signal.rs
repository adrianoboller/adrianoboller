//! Signal pelo signal-cli REST API (o servico local que fala o protocolo do Signal).
//!
//! O `GET /v1/receive/{numero}` ENTREGA E ESQUECE: o que ele devolveu nao volta. Se o laco
//! criasse a tarefa direto da resposta, uma queda entre receber e criar perderia a
//! mensagem -- entao o que chega vai primeiro para a `Caixa` em disco, e o laco le da caixa
//! pelo cursor. So conversa direta (o numero de quem escreveu); grupo fica de fora.
//!
//! O signal-cli precisa estar no modo `normal` ou `native` (no `json-rpc` o receive vira
//! WebSocket). Sem token: o servico escuta em loopback; se houver um proxy na frente, o
//! token vai como Bearer pelo broker.

use super::caixa::Caixa;
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use reqwest::blocking::RequestBuilder;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub struct Signal {
    http: Http,
    numero: String,
    caixa: Arc<Caixa>,
    token: Option<Credencial>,
}

impl Signal {
    pub fn novo(
        caixa: Arc<Caixa>,
        numero: String,
        token: Option<Credencial>,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(base, politica)?,
            numero,
            caixa,
            token,
        })
    }

    fn pedir(
        &self,
        uso: &str,
        f: impl Fn() -> Result<RequestBuilder, String>,
    ) -> Result<Value, String> {
        match &self.token {
            Some(c) => c.com(uso, |t| self.http.json(f()?.bearer_auth(t))),
            None => self.http.json(f()?),
        }
    }
}

/// O envelope do signal-cli: so `dataMessage` de conversa direta e mensagem.
pub fn mensagens_do_signal(v: &Value) -> Vec<Mensagem> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| {
            let e = &x["envelope"];
            let d = &e["dataMessage"];
            if d.is_null() || !d["groupInfo"].is_null() {
                return None;
            }
            let de = e["sourceNumber"]
                .as_str()
                .or_else(|| e["source"].as_str())?
                .to_string();
            Some(Mensagem {
                conversa: de.clone(),
                autor: de.clone(),
                id: format!("{de}:{}", e["timestamp"].as_i64().unwrap_or(0)),
                texto: d["message"].as_str().map(str::to_string),
            })
        })
        .collect()
}

impl Provedor for Signal {
    fn nome(&self) -> &str {
        "signal"
    }

    fn limite(&self) -> (usize, Unidade) {
        (2000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let url = self.http.url(&format!(
            "/v1/receive/{}",
            super::http::trecho(&self.numero)
        ));
        let v = self.pedir("receive", || {
            Ok(self
                .http
                .cliente()?
                .get(&url)
                .query(&[("timeout", espera_seg.min(25).to_string())]))
        })?;
        let novas = mensagens_do_signal(&v);
        if !novas.is_empty() {
            self.caixa
                .anexar(novas.into_iter().map(|m| (m, Value::Null)).collect())?;
        }
        self.caixa.ler_desde(cursor, Duration::ZERO)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let v =
            self.pedir("send", || {
                Ok(self.http.cliente()?.post(self.http.url("/v2/send")).json(
                    &json!({"message": texto, "number": self.numero, "recipients": [conversa]}),
                ))
            })?;
        Ok(v["timestamp"]
            .as_str()
            .map(str::to_string)
            .or_else(|| v["timestamp"].as_i64().map(|t| t.to_string()))
            .unwrap_or_default())
    }
}
