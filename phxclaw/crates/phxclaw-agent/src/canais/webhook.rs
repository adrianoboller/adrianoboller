//! Webhook generico: qualquer sistema fala com o agente por HTTP assinado, nos dois
//! sentidos, com o mesmo segredo.
//!
//! Entrada: `POST /canais/webhook/webhook` com `{"conversa","id","texto"}`, o cabecalho
//! `X-PhxClaw-Carimbo` (segundos Unix) e `X-PhxClaw-Assinatura: sha256=<hex>` sobre
//! `carimbo + "." + corpo`. O carimbo entra na conta para que um pedido capturado nao valha
//! para sempre: fora de 5 minutos e recusa, e dentro deles o `id` repetido ja e descartado
//! pela caixa.
//!
//! Saida: `POST` na URL configurada com `{"conversa","texto","id"}`, assinado igual, para o
//! outro lado conferir que foi o agente.

use super::caixa::{Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, erro_interno};
use super::cripto::{hex, hmac_sha256, iguais};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const JANELA_SEG: i64 = 300;

pub fn assinar(segredo: &str, carimbo: i64, corpo: &[u8]) -> String {
    let mut msg = format!("{carimbo}.").into_bytes();
    msg.extend_from_slice(corpo);
    format!("sha256={}", hex(&hmac_sha256(segredo.as_bytes(), &msg)))
}

pub struct Webhook {
    caixa: Arc<Caixa>,
    segredo: Credencial,
    saida: Option<Http>,
}

impl Webhook {
    /// `saida` e a URL que recebe as respostas; sem ela o canal so ouve.
    pub fn novo(
        caixa: Arc<Caixa>,
        segredo: Credencial,
        saida: Option<(&str, ProviderEndpointPolicy)>,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            segredo,
            saida: saida.map(|(u, p)| Http::novo(u, p)).transpose()?,
        })
    }
}

impl Recebedor for Webhook {
    fn nome(&self) -> &str {
        "webhook"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        let carimbo: i64 = p
            .cabecalho("x-phxclaw-carimbo")
            .and_then(|c| c.trim().parse().ok())
            .ok_or((401, "falta X-PhxClaw-Carimbo".to_string()))?;
        if (chrono::Utc::now().timestamp() - carimbo).abs() > JANELA_SEG {
            return Err((401, "carimbo fora da janela".into()));
        }
        let dada = p
            .cabecalho("x-phxclaw-assinatura")
            .ok_or((401, "falta X-PhxClaw-Assinatura".to_string()))?
            .trim()
            .to_string();
        let ok = self
            .segredo
            .com("verify", |s| {
                Ok(iguais(
                    dada.as_bytes(),
                    assinar(s, carimbo, &p.corpo).as_bytes(),
                ))
            })
            .map_err(erro_interno)?;
        if !ok {
            return Err((401, "assinatura nao confere".into()));
        }
        let v: Value = serde_json::from_slice(&p.corpo)
            .map_err(|e| (400, format!("corpo nao e JSON: {e}")))?;
        let conversa = v["conversa"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or((400, "falta conversa".to_string()))?;
        let id = v["id"].as_str().filter(|s| !s.trim().is_empty()).ok_or((
            400,
            "falta id (e ele que impede reprocessar o reenvio)".to_string(),
        ))?;
        let m = Mensagem {
            conversa: conversa.to_string(),
            autor: v["autor"].as_str().unwrap_or(conversa).to_string(),
            id: id.to_string(),
            texto: v["texto"].as_str().map(str::to_string),
        };
        let n = self
            .caixa
            .anexar(vec![(m, Value::Null)])
            .map_err(erro_interno)?;
        Ok(RespostaWebhook::json(
            json!({"aceita": n == 1, "repetida": n == 0}),
        ))
    }
}

impl Provedor for Webhook {
    fn nome(&self) -> &str {
        "webhook"
    }

    fn limite(&self) -> (usize, Unidade) {
        (16_000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.caixa
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let http = self
            .saida
            .as_ref()
            .ok_or("webhook sem URL de saida: o canal so ouve")?;
        let id = phxclaw_types::new_uuid_v7().to_string();
        let corpo = json!({"conversa": conversa, "texto": texto, "id": id}).to_string();
        let carimbo = chrono::Utc::now().timestamp();
        self.segredo.com("send", |s| {
            http.json(
                http.cliente()?
                    .post(http.base())
                    .header("content-type", "application/json")
                    .header("x-phxclaw-carimbo", carimbo.to_string())
                    .header(
                        "x-phxclaw-assinatura",
                        assinar(s, carimbo, corpo.as_bytes()),
                    )
                    .body(corpo.clone()),
            )
        })?;
        Ok(id)
    }
}
