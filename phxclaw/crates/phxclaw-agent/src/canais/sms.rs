//! SMS pelo Twilio: o SMS chega por webhook (formulario) e a resposta sai pela API REST.
//!
//! A assinatura do Twilio e `base64(HMAC-SHA1(auth token, URL + chaves e valores do
//! formulario em ordem de chave))`, e a URL e a que o TWILIO chamou -- atras de proxy ela
//! nao e a que o agente ve. Por isso a URL publica e configuracao
//! (`PHXCLAW_SMS_URL_PUBLICA`), e nao deducao do pedido: deduzir erraria calado, e todo SMS
//! seria recusado como forjado.

use super::caixa::{
    Caixa, PedidoWebhook, Recebedor, Recusa, RespostaWebhook, erro_interno, parametros,
};
use super::cripto::{base64, hmac_sha1, iguais};
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub const BASE: &str = "https://api.twilio.com";

pub struct Sms {
    caixa: Arc<Caixa>,
    /// O auth token: assina o webhook e autentica a API.
    token: Credencial,
    conta: String,
    numero: String,
    url_publica: String,
    http: Http,
}

/// A conta da assinatura do Twilio, separada para o teste montar o pedido como ele monta.
pub fn assinatura_twilio(token: &str, url: &str, form: &[(String, String)]) -> String {
    let mut pares = form.to_vec();
    pares.sort();
    let mut dado = url.to_string();
    for (k, v) in pares {
        dado.push_str(&k);
        dado.push_str(&v);
    }
    base64(&hmac_sha1(token.as_bytes(), dado.as_bytes()))
}

impl Sms {
    pub fn novo(
        caixa: Arc<Caixa>,
        token: Credencial,
        conta: String,
        numero: String,
        url_publica: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            token,
            conta,
            numero,
            url_publica,
            http: Http::novo(base, politica)?,
        })
    }
}

impl Recebedor for Sms {
    fn nome(&self) -> &str {
        "sms"
    }

    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        let corpo = String::from_utf8_lossy(&p.corpo).to_string();
        let form = parametros(&corpo);
        let dada = p
            .cabecalho("x-twilio-signature")
            .ok_or((401, "falta X-Twilio-Signature".to_string()))?
            .trim()
            .to_string();
        let ok = self
            .token
            .com("verify", |t| {
                let esperada = assinatura_twilio(t, &self.url_publica, &form);
                Ok(iguais(dada.as_bytes(), esperada.as_bytes()))
            })
            .map_err(erro_interno)?;
        if !ok {
            return Err((401, "assinatura nao confere".into()));
        }
        let campo = |k: &str| form.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone());
        let de = campo("From").ok_or((400, "falta From".to_string()))?;
        let m = Mensagem {
            conversa: de.clone(),
            autor: de,
            id: campo("MessageSid").unwrap_or_default(),
            texto: campo("Body"),
        };
        self.caixa
            .anexar(vec![(m, Value::Null)])
            .map_err(erro_interno)?;
        // TwiML vazio: nada de resposta automatica, a resposta e a da tarefa.
        Ok(RespostaWebhook {
            status: 200,
            tipo: "text/xml",
            corpo: "<Response/>".into(),
        })
    }
}

impl Provedor for Sms {
    fn nome(&self) -> &str {
        "sms"
    }

    /// 1600 e o teto do Twilio por mensagem (ele mesmo parte em segmentos de SMS).
    fn limite(&self) -> (usize, Unidade) {
        (1600, Unidade::Caractere)
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
                    .post(self.http.url(&format!(
                        "/2010-04-01/Accounts/{}/Messages.json",
                        super::http::trecho(&self.conta)
                    )))
                    .basic_auth(&self.conta, Some(t))
                    .form(&[("To", conversa), ("From", &self.numero), ("Body", texto)]),
            )?;
            Ok(v["sid"].as_str().unwrap_or("").to_string())
        })
    }
}
