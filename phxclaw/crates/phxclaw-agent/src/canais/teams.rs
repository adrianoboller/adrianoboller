//! Microsoft Teams pelo Bot Framework: a mensagem chega por webhook (uma Activity) e a
//! resposta vai para o `serviceUrl` que a propria Activity trouxe.
//!
//! Duas decisoes que valem saber:
//! - o Bot Framework assina o webhook com JWT RS256, e conferir RS256 pede RSA, que o
//!   agente nao tem sem crate nova. A entrada confere uma chave na URL (`?chave=`, guardada
//!   no broker, a URL de mensagens e o dono do bot quem escreve no portal). E mais fraca
//!   que o JWT -- quem vir a URL entra --, e por isso o canal conta como PARCIAL;
//! - o `serviceUrl` vem de fora, e responder a ele sem conferir seria um SSRF de graca:
//!   ele passa pela politica de destinos (`PHXCLAW_TEAMS_SERVICOS`, padrao
//!   `https://smba.trafficmanager.net`) antes de cada envio.
//!
//! O token de saida (client credentials) e pedido a cada envio e nao fica guardado: segredo
//! derivado tambem e segredo, e o broker e o unico lugar onde segredo mora.

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, conferir_chave_na_url, erro_interno,
};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const LOGIN: &str = "https://login.microsoftonline.com";
pub const SERVICO: &str = "https://smba.trafficmanager.net";

pub struct Teams {
    caixa: Arc<Caixa>,
    chave_url: Credencial,
    segredo_app: Credencial,
    app_id: String,
    http: Http,
}

impl Teams {
    /// `politica` precisa aceitar a origem do login e as dos servicos.
    pub fn novo(
        caixa: Arc<Caixa>,
        chave_url: Credencial,
        segredo_app: Credencial,
        app_id: String,
        login: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            chave_url,
            segredo_app,
            app_id,
            http: Http::novo(login, politica)?,
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

impl Recebedor for Teams {
    fn nome(&self) -> &str {
        "teams"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        self.chave_url
            .com("verify", |s| Ok(conferir_chave_na_url(p, s)))
            .map_err(erro_interno)??;
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
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
