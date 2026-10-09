//! Microsoft Teams pelo Bot Framework: a mensagem chega por webhook (uma Activity) e a
//! resposta vai para o `serviceUrl` que a propria Activity trouxe.
//!
//! Tres decisoes que valem saber:
//! - a entrada confere o JWT RS256 que o Bot Framework poe no `Authorization` (`jwt.rs` e
//!   `rsa.rs`), nos sete passos da Microsoft: Bearer, assinatura pela chave do JWKS, `iss`
//!   `https://api.botframework.com`, `aud` = App ID, validade com 5 min de folga, o claim
//!   `serviceUrl` IGUAL ao da Activity e a chave endossada para o `channelId` da Activity
//!   (403 se nao). Nao ha como desligar: sem token valido, 401. A chave na URL
//!   (`?chave=`), que era a unica guarda antes do RSA, continua conferida SE configurada --
//!   soma, nao substitui;
//! - o `serviceUrl` vem de fora, e responder a ele sem conferir seria um SSRF de graca:
//!   ele passa pela politica de destinos (`PHXCLAW_TEAMS_SERVICOS`, padrao
//!   `https://smba.trafficmanager.net`) antes de cada envio -- e agora tambem tem de estar
//!   assinado no token, que e o que impede quem tem um token velho de trocar o destino;
//! - o caminho do Emulador (outro emissor, `login.microsoftonline.com`) fica fora: ele so
//!   existe em desenvolvimento, e aceitar dois emissores sem dizer seria aceitar o pior.
//!
//! O token de saida (client credentials) e pedido a cada envio e nao fica guardado: segredo
//! derivado tambem e segredo, e o broker e o unico lugar onde segredo mora.

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, conferir_chave_na_url, erro_interno,
};
use super::http::{Credencial, Http};
use super::jwt::{self, Conferido, Exigido, Jwks, Motivo};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const LOGIN: &str = "https://login.microsoftonline.com";
pub const SERVICO: &str = "https://smba.trafficmanager.net";
/// O emissor e o JWKS do canal do Bot Framework (o `jwks_uri` do
/// `login.botframework.com/v1/.well-known/openidconfiguration`).
pub const EMISSOR: &str = "https://api.botframework.com";
pub const JWKS: &str = "https://login.botframework.com/v1/.well-known/keys";

pub struct Teams {
    caixa: Arc<Caixa>,
    chave_url: Option<Credencial>,
    segredo_app: Credencial,
    app_id: String,
    http: Http,
    jwks: Jwks,
}

impl Teams {
    /// `politica` precisa aceitar a origem do login e as dos servicos; o `jwks` tem a
    /// politica dele (so a origem do JWKS).
    pub fn novo(
        caixa: Arc<Caixa>,
        chave_url: Option<Credencial>,
        segredo_app: Credencial,
        app_id: String,
        login: &str,
        politica: ProviderEndpointPolicy,
        jwks: Jwks,
    ) -> Result<Self, String> {
        if app_id.trim().is_empty() {
            return Err("App ID vazio: sem ele nao ha audiencia para conferir o token".into());
        }
        Ok(Self {
            caixa,
            chave_url,
            segredo_app,
            app_id,
            http: Http::novo(login, politica)?,
            jwks,
        })
    }

    /// O pedido de token de client credentials (o segredo do app vai no corpo).
    fn login(&self, segredo: &str) -> Result<String, String> {
        let v = self.http.json(
            self.http
                .cliente()?
                .post(self.http.url("/botframework.com/oauth2/v2.0/token"))
                .form(&[
                    ("grant_type", "client_credentials"),
                    ("client_id", self.app_id.as_str()),
                    ("client_secret", segredo),
                    ("scope", "https://api.botframework.com/.default"),
                ]),
        )?;
        v["access_token"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "login sem access_token".to_string())
    }
}

/// A Activity de mensagem. A mencao ao bot (`<at>Nome</at>`) sai do texto: e como o Teams
/// marca "falaram comigo" num canal, nao parte do pedido.
pub fn mensagem_da_activity(v: &Value) -> Option<(Mensagem, Value)> {
    if v["type"] != "message" {
        return None;
    }
    let conversa = v["conversation"]["id"].as_str()?.to_string();
    let mut texto = v["text"].as_str().map(str::to_string);
    if let Some(t) = &mut texto {
        while let (Some(a), Some(b)) = (t.find("<at>"), t.find("</at>")) {
            if b < a {
                break;
            }
            t.replace_range(a..b + 5, "");
        }
        *t = t.trim().to_string();
    }
    Some((
        Mensagem {
            conversa,
            autor: v["from"]["id"].as_str().unwrap_or("").to_string(),
            id: v["id"].as_str().unwrap_or("").to_string(),
            texto,
        },
        json!({"serviceUrl": v["serviceUrl"]}),
    ))
}

/// O que o token tem de dizer sobre ESTA Activity: o `serviceUrl` assinado igual ao do
/// corpo (sem isto, um token valido de uma conversa servia para mandar a resposta de outra
/// para onde o corpo quisesse) e a chave endossada para o `channelId`.
pub fn conferir_activity(t: &Conferido, activity: &Value) -> Result<(), Motivo> {
    let assinado = t.claims["serviceUrl"].as_str();
    if assinado.is_none() || assinado != activity["serviceUrl"].as_str() {
        return Err(Motivo::NaoVaiAqui);
    }
    let canal = activity["channelId"].as_str().ok_or(Motivo::NaoVaiAqui)?;
    if !t.endossos.iter().any(|e| e == canal) {
        return Err(Motivo::NaoVaiAqui);
    }
    Ok(())
}

impl Recebedor for Teams {
    fn nome(&self) -> &str {
        "teams"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        if let Some(c) = &self.chave_url {
            c.com("verify", |s| Ok(conferir_chave_na_url(p, s)))
                .map_err(erro_interno)??;
        }
        let token = jwt::bearer(p).ok_or_else(|| Motivo::SemToken.recusa())?;
        let exigido = Exigido {
            emissores: &[EMISSOR],
            audiencia: &self.app_id,
        };
        let t = jwt::conferir(token, &self.jwks, &exigido, jwt::agora()).map_err(Motivo::recusa)?;
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
        conferir_activity(&t, &v).map_err(Motivo::recusa)?;
        if let Some(m) = mensagem_da_activity(&v) {
            self.caixa.anexar(vec![m]).map_err(erro_interno)?;
        }
        Ok(RespostaWebhook::ok())
    }
}

impl Provedor for Teams {
    fn nome(&self) -> &str {
        "teams"
    }

    /// O Teams recusa mensagem acima de ~28 KB; 20 mil bytes deixam folga para o envelope.
    fn limite(&self) -> (usize, Unidade) {
        (20_000, Unidade::Byte)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let servico = self
            .caixa
            .extra_de(conversa)
            .and_then(|e| e["serviceUrl"].as_str().map(str::to_string))
            .ok_or("conversa sem serviceUrl: o Teams so deixa responder a quem ja escreveu")?;
        let servico = servico.trim_end_matches('/').to_string();
        self.http.conferir(&servico)?;
        let url = format!(
            "{servico}/v3/conversations/{}/activities",
            super::http::trecho(conversa)
        );
        let v = self.segredo_app.com_derivado(
            "send",
            |s| self.login(s),
            |token| {
                self.http.json(
                    self.http
                        .cliente()?
                        .post(&url)
                        .bearer_auth(token)
                        .json(&json!({"type": "message", "text": texto})),
                )
            },
        )?;
        Ok(v["id"].as_str().unwrap_or("").to_string())
    }
}
