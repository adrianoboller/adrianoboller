//! Azure Key Vault: acesso do Entra ID por client credentials (RFC 6749 §4.4) -- o MESMO
//! pedido de token do `oauth.rs` (`pedir_token_por` e `form_cliente`), pela `Saida` presa ao
//! endpoint de token declarado -- e `GET <cofre>/secrets/<nome>[/<versao>]?api-version=7.4`.
//! O acesso fica so em memoria ate vencer; um 401 o esquece e pede outro, uma vez.

use super::{
    AZURE_SEGREDO, CfgAzure, ConfigCofres, Saida, TokenEmMemoria, base, escolher_campo, json_de,
    limpar, motivo_do_status, segmento,
};
use crate::oauth::{self, Concessao, ConfigOauth};
use phxclaw_http_client::HttpRequestSpec;
use phxclaw_secret_broker::SecretValue;
use phxclaw_secret_broker::cofre::{BoxFut, CofreExterno, ReferenciaExterna};
use std::path::{Path, PathBuf};

/// O escopo do Key Vault na nuvem publica do Azure.
pub const ESCOPO: &str = "https://vault.azure.net/.default";
pub const VERSAO_DA_API: &str = "7.4";

pub struct Azure {
    raiz: PathBuf,
    url: String,
    oauth: ConfigOauth,
    saida_cofre: Saida,
    saida_token: Saida,
    acesso: TokenEmMemoria,
}

impl Azure {
    pub fn novo(raiz: &Path, a: &CfgAzure, cfg: &ConfigCofres) -> Result<Self, String> {
        if a.cliente_id.trim().is_empty() {
            return Err("Azure sem cofres.azure.cliente_id".into());
        }
        let token_url = match a.token_url.as_deref().filter(|t| !t.trim().is_empty()) {
            Some(t) => t.trim().to_string(),
            None => {
                let t = a.tenant.trim();
                if t.is_empty()
                    || !t
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
                {
                    return Err(format!("cofres.azure.tenant invalido: {t:?}"));
                }
                format!("https://login.microsoftonline.com/{t}/oauth2/v2.0/token")
            }
        };
        let oauth = ConfigOauth {
            token: token_url.clone(),
            cliente_id: a.cliente_id.trim().to_string(),
            escopos: vec![ESCOPO.into()],
            concessao: Concessao::Cliente,
            ..Default::default()
        };
        oauth.validar()?;
        Ok(Self {
            raiz: raiz.to_path_buf(),
            url: a.url.trim().trim_end_matches('/').to_string(),
            saida_cofre: Saida::nova(&a.url, cfg)?,
            saida_token: Saida::nova(&token_url, cfg)?,
            oauth,
            acesso: TokenEmMemoria::default(),
        })
    }

    async fn acesso(&self) -> Result<SecretValue, String> {
        if let Some(t) = self.acesso.valido() {
            return Ok(t);
        }
        let segredo = base(&self.raiz, &AZURE_SEGREDO)?;
        let r = oauth::pedir_token_por(
            &self.oauth,
            oauth::form_cliente(&self.oauth),
            Some(segredo.expose()),
            |spec| self.saida_token.enviar(spec),
        )
        .await
        .map_err(|e| limpar(format!("Entra ID: {e}"), &[&segredo]))?;
        self.acesso.guardar(r.acesso.clone(), r.validade);
        Ok(r.acesso)
    }

    async fn ler_valor(&self, r: &ReferenciaExterna) -> Result<SecretValue, String> {
        let mut url = format!("{}/secrets/{}", self.url, segmento(&r.caminho, "nome")?);
        if let Some(v) = &r.versao {
            url.push('/');
            url.push_str(&segmento(v, "versao")?);
        }
        url.push_str(&format!("?api-version={VERSAO_DA_API}"));
        for tentativa in 0..2 {
            let acesso = self.acesso().await?;
            let mut spec = HttpRequestSpec::get(url.clone());
            spec.headers.insert(
                "Authorization".into(),
                format!("Bearer {}", acesso.expose()),
            );
            let resp = self.saida_cofre.enviar(spec).await;
            let resp = resp.map_err(|e| limpar(e, &[&acesso]))?;
            if resp.status == 401 && tentativa == 0 {
                self.acesso.esquecer();
                continue;
            }
            let v = json_de(&resp);
            if resp.status != 200 {
                let codigo = v["error"]["code"].as_str().map(str::to_string);
                return Err(motivo_do_status(resp.status, codigo));
            }
            let valor = v["value"]
                .as_str()
                .ok_or("o Key Vault respondeu sem 'value'")?;
            return escolher_campo(valor, r.campo.as_deref());
        }
        Err("o Key Vault recusou o acesso de novo depois de renovar (401)".into())
    }
}

impl CofreExterno for Azure {
    fn tipo(&self) -> &'static str {
        "azure"
    }

    fn ler<'a>(&'a self, r: &'a ReferenciaExterna) -> BoxFut<'a, Result<SecretValue, String>> {
        Box::pin(self.ler_valor(r))
    }
}
