//! Google Chat: o evento chega por webhook do app (endpoint HTTP) e a resposta sai pelo
//! webhook de entrada do espaco.
//!
//! A entrada confere o JWT RS256 do `Authorization: Bearer` (`jwt.rs` e `rsa.rs`), no modo
//! que a audiencia configurada diz -- os dois que o Google documenta, e nunca os dois sem
//! dizer qual:
//! - `PHXCLAW_GOOGLECHAT_AUDIENCIA` = numero do projeto: o token e da conta de servico do
//!   Chat (`iss` = `chat@system.gserviceaccount.com`), chaves no JWKS dela;
//! - `PHXCLAW_GOOGLECHAT_AUDIENCIA` = URL `https://` do endpoint: e um ID token OIDC
//!   (`iss` = `accounts.google.com`), chaves em `oauth2/v3/certs`, e o `email` assinado tem
//!   de ser o da conta do Chat com `email_verified` -- sem isso, qualquer ID token do
//!   Google com a nossa URL de audiencia entraria.
//!
//! Falhou, 401 sem detalhe. A chave na URL (`?chave=`), que era a unica guarda antes do
//! RSA, continua conferida SE configurada -- soma, nao substitui.
//!
//! O que continua PARCIAL: responder pela API (`spaces.messages.create`) exige conta de
//! servico, que ASSINA JWT RS256 -- e assinar com a reducao bit a bit custa ~1,2 s por token
//! (parecer do pesquisador), decisao da onda de assinatura. A saida usa o webhook de entrada
//! do espaco -- a URL inteira e o segredo, guardado no broker --, e por isso so fala com o
//! espaco desse webhook.

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, conferir_chave_na_url, erro_interno,
};
use super::http::{Credencial, Http};
use super::jwt::{self, Exigido, Jwks, Motivo};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://chat.googleapis.com";
/// A conta de servico que assina o token do Chat (modo numero do projeto) e o JWKS dela.
pub const CONTA_DO_CHAT: &str = "chat@system.gserviceaccount.com";
pub const JWKS_PROJETO: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/chat@system.gserviceaccount.com";
/// O emissor e as chaves do ID token OIDC (modo URL do endpoint).
pub const EMISSORES_OIDC: &[&str] = &["https://accounts.google.com", "accounts.google.com"];
pub const JWKS_OIDC: &str = "https://www.googleapis.com/oauth2/v3/certs";

/// A conferencia do token de entrada, com o modo decidido pela audiencia.
pub struct Verificacao {
    audiencia: String,
    jwks: Jwks,
}

impl Verificacao {
    /// `jwks`: a URL das chaves; `None` e a oficial do modo.
    pub fn nova(audiencia: String, jwks: Option<&str>) -> Result<Self, String> {
        Self::com_jwks(audiencia, |padrao| Jwks::novo(jwks.unwrap_or(padrao)))
    }

    /// Com o JWKS montado por quem chama (o teste o monta com prazos curtos).
    pub fn com_jwks(
        audiencia: String,
        jwks: impl FnOnce(&str) -> Result<Jwks, String>,
    ) -> Result<Self, String> {
        let audiencia = audiencia.trim().to_string();
        if audiencia.is_empty() {
            return Err("audiencia vazia: numero do projeto ou URL do endpoint".into());
        }
        let padrao = if modo_oidc(&audiencia) {
            JWKS_OIDC
        } else {
            JWKS_PROJETO
        };
        Ok(Self {
            jwks: jwks(padrao)?,
            audiencia,
        })
    }

    pub fn conferir(&self, p: &PedidoWebhook) -> Result<(), Motivo> {
        let token = jwt::bearer(p).ok_or(Motivo::SemToken)?;
        let oidc = modo_oidc(&self.audiencia);
        let exigido = Exigido {
            emissores: if oidc {
                EMISSORES_OIDC
            } else {
                &[CONTA_DO_CHAT]
            },
            audiencia: &self.audiencia,
        };
        let t = jwt::conferir(token, &self.jwks, &exigido, jwt::agora())?;
        if oidc && (t.claims["email"] != CONTA_DO_CHAT || t.claims["email_verified"] != true) {
            return Err(Motivo::NaoVaiAqui);
        }
        Ok(())
    }
}

/// A audiencia que e URL e a do ID token OIDC; o numero do projeto nao tem esquema.
fn modo_oidc(audiencia: &str) -> bool {
    audiencia.starts_with("https://")
}

pub struct GoogleChat {
    caixa: Arc<Caixa>,
    chave_url: Option<Credencial>,
    verificacao: Verificacao,
    /// A URL do webhook de entrada do espaco (com `key` e `token`), no broker.
    saida: Credencial,
    espaco: String,
    http: Http,
}

impl GoogleChat {
    pub fn novo(
        caixa: Arc<Caixa>,
        chave_url: Option<Credencial>,
        saida: Credencial,
        espaco: String,
        base: &str,
        politica: ProviderEndpointPolicy,
        verificacao: Verificacao,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            chave_url,
            verificacao,
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
        if let Some(c) = &self.chave_url {
            c.com("verify", |s| Ok(conferir_chave_na_url(p, s)))
                .map_err(erro_interno)??;
        }
        // O Google pede 401 para toda falha de token, inclusive o `email` errado do ID
        // token (que no Teams seria 403); so a falta de chave do nosso lado sai 503.
        self.verificacao.conferir(p).map_err(|m| match m {
            Motivo::NaoVaiAqui => Motivo::Assinatura.recusa(),
            outro => outro.recusa(),
        })?;
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
