//! WhatsApp (Cloud API) e Messenger: os dois sao da Meta, chegam por webhook com a mesma
//! assinatura (`X-Hub-Signature-256: sha256=HMAC(app secret, corpo)`) e o mesmo aperto de
//! mao por GET (`hub.verify_token` -> `hub.challenge`). A conferencia e uma so, aqui; o que
//! difere e so onde a mensagem mora no JSON e como se responde.
//!
//! O WhatsApp responde pelo `WhatsAppProvider` do `phxclaw-channel-providers`; o Messenger
//! pela Send API (`/me/messages`). Conversa e o id de quem escreveu (wa_id ou PSID).

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, conferir_hmac_hex, erro_interno,
};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Preguicoso, Provedor, Unidade, enviar_pelo};
use phxclaw_channel_providers::{ProviderEndpointPolicy, WhatsAppProvider};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://graph.facebook.com";
pub const VERSAO: &str = "v21.0";

/// Os segredos que a Meta pede de quem recebe: o app secret assina o corpo, o verify token
/// so serve ao aperto de mao.
pub struct Assinatura {
    pub app_secret: Credencial,
    pub verify_token: Credencial,
}

impl Assinatura {
    fn conferir(&self, p: &PedidoWebhook) -> Result<(), Recusa> {
        self.app_secret
            .com("verify", |s| {
                Ok(conferir_hmac_hex(p, "x-hub-signature-256", "sha256=", s))
            })
            .map_err(erro_interno)?
    }

    fn aperto(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        if p.parametro("hub.mode").as_deref() != Some("subscribe") {
            return Err((400, "hub.mode".into()));
        }
        let dado = p.parametro("hub.verify_token").unwrap_or_default();
        let ok = self
            .verify_token
            .com("verify", |s| {
                Ok(super::cripto::iguais(dado.as_bytes(), s.as_bytes()))
            })
            .map_err(erro_interno)?;
        if !ok {
            return Err((403, "verify_token nao confere".into()));
        }
        Ok(RespostaWebhook::texto(
            p.parametro("hub.challenge").unwrap_or_default(),
        ))
    }
}

fn corpo_json(p: &PedidoWebhook) -> Result<Value, Recusa> {
    serde_json::from_slice(&p.corpo).map_err(|e| (400, format!("corpo nao e JSON: {e}")))
}

pub struct WhatsApp {
    caixa: Arc<Caixa>,
    assinatura: Assinatura,
    envio: Preguicoso<WhatsAppProvider>,
}

impl WhatsApp {
    pub fn novo(
        caixa: Arc<Caixa>,
        assinatura: Assinatura,
        token: Credencial,
        phone_number_id: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        politica.validate(base).map_err(|e| e.to_string())?;
        let base = base.to_string();
        let (broker, token) = (token.broker(), token.id());
        Ok(Self {
            caixa,
            assinatura,
            envio: Preguicoso::novo(move || {
                WhatsAppProvider::new_with_origin(
                    broker.clone(),
                    token,
                    phone_number_id.clone(),
                    VERSAO,
                    base.clone(),
                    politica.clone(),
                )
                .map_err(|e| e.to_string())
            }),
        })
    }
}

/// `entry[].changes[].value.messages[]`. Status de entrega (lida, entregue) vem no mesmo
/// webhook, em `statuses`, e nao e mensagem.
pub fn mensagens_do_whatsapp(v: &Value) -> Vec<Mensagem> {
    let mut saida = Vec::new();
    for e in v["entry"].as_array().into_iter().flatten() {
        for c in e["changes"].as_array().into_iter().flatten() {
            for m in c["value"]["messages"].as_array().into_iter().flatten() {
                let de = m["from"].as_str().unwrap_or("").to_string();
                saida.push(Mensagem {
                    conversa: de.clone(),
                    autor: de,
                    id: m["id"].as_str().unwrap_or("").to_string(),
                    texto: (m["type"] == "text")
                        .then(|| m["text"]["body"].as_str().map(str::to_string))
                        .flatten(),
                });
            }
        }
    }
    saida
}

impl Recebedor for WhatsApp {
    fn nome(&self) -> &str {
        "whatsapp"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.assinatura.conferir(p)?;
        let msgs = mensagens_do_whatsapp(&corpo_json(p)?);
        self.caixa
            .anexar(msgs.into_iter().map(|m| (m, Value::Null)).collect())
            .map_err(erro_interno)?;
        Ok(RespostaWebhook::ok())
    }

    fn get(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.assinatura.aperto(p)
    }
}

impl Provedor for WhatsApp {
    fn nome(&self) -> &str {
        "whatsapp"
    }

    fn limite(&self) -> (usize, Unidade) {
        (4096, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        enviar_pelo(self.envio.obter()?, conversa, texto)
    }
}

pub struct Messenger {
    caixa: Arc<Caixa>,
    assinatura: Assinatura,
    http: Http,
    token: Credencial,
}

impl Messenger {
    pub fn novo(
        caixa: Arc<Caixa>,
        assinatura: Assinatura,
        token: Credencial,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            assinatura,
            http: Http::novo(base, politica)?,
            token,
        })
    }
}

/// `entry[].messaging[]`. O eco da propria pagina (`is_echo`) nao e mensagem de ninguem.
pub fn mensagens_do_messenger(v: &Value) -> Vec<Mensagem> {
    let mut saida = Vec::new();
    for e in v["entry"].as_array().into_iter().flatten() {
        for m in e["messaging"].as_array().into_iter().flatten() {
            let msg = &m["message"];
            if msg.is_null() || msg["is_echo"].as_bool() == Some(true) {
                continue;
            }
            let de = m["sender"]["id"].as_str().unwrap_or("").to_string();
            saida.push(Mensagem {
                conversa: de.clone(),
                autor: de,
                id: msg["mid"].as_str().unwrap_or("").to_string(),
                texto: msg["text"].as_str().map(str::to_string),
            });
        }
    }
    saida
}

impl Recebedor for Messenger {
    fn nome(&self) -> &str {
        "messenger"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.assinatura.conferir(p)?;
        let msgs = mensagens_do_messenger(&corpo_json(p)?);
        self.caixa
            .anexar(msgs.into_iter().map(|m| (m, Value::Null)).collect())
            .map_err(erro_interno)?;
        Ok(RespostaWebhook::ok())
    }

    fn get(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.assinatura.aperto(p)
    }
}

impl Provedor for Messenger {
    fn nome(&self) -> &str {
        "messenger"
    }

    fn limite(&self) -> (usize, Unidade) {
        (2000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        self.token.com("send", |token| {
            let v = self.http.json(
                self.http
                    .cliente()?
                    .post(self.http.url(&format!("/{VERSAO}/me/messages")))
                    .bearer_auth(token)
                    .json(&json!({"recipient": {"id": conversa},
                        "messaging_type": "RESPONSE", "message": {"text": texto}})),
            )?;
            Ok(v["message_id"].as_str().unwrap_or("").to_string())
        })
    }
}
