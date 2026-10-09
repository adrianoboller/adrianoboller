//! GCP Secret Manager: a conta de servico (o JSON guardado no broker local) assina um JWT
//! RS256 com o RSA da casa (`canais::rsa::ChavePrivada`, `canais::jwt::assinar_rs256`), o
//! endpoint de token o troca por acesso (RFC 7523, `grant_type=...jwt-bearer`, pelo mesmo
//! `oauth::pedir_token_por`), e `GET .../secrets/<nome>/versions/<versao>:access` devolve o
//! valor em base64.
//!
//! O `aud` do JWT e o endpoint de token DECLARADO (`cofres.gcp.token_url`, padrao o do
//! Google), nao o `token_uri` que vem dentro do JSON da conta: o arquivo e dado, e um
//! `token_uri` trocado nele mandaria a assercao assinada para outro lugar.

use super::{
    CfgGcp, ConfigCofres, GCP_CONTA, Saida, TokenEmMemoria, base, escolher_campo, json_de, limpar,
    motivo_do_status, segmento,
};
use crate::canais::jwt::{agora, assinar_rs256};
use crate::canais::rsa::ChavePrivada;
use crate::oauth::{self, ConfigOauth};
use base64::Engine;
use phxclaw_http_client::HttpRequestSpec;
use phxclaw_secret_broker::SecretValue;
use phxclaw_secret_broker::cofre::{BoxFut, CofreExterno, ReferenciaExterna};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const ESCOPO: &str = "https://www.googleapis.com/auth/cloud-platform";
pub const CONCESSAO: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
pub const URL_PADRAO: &str = "https://secretmanager.googleapis.com";
pub const TOKEN_PADRAO: &str = "https://oauth2.googleapis.com/token";
/// A validade pedida para a assercao: o maximo que o Google aceita.
const VALIDADE_SEG: i64 = 3600;

pub struct Gcp {
    raiz: PathBuf,
    url: String,
    token_url: String,
    projeto: Option<String>,
    saida_api: Saida,
    saida_token: Saida,
    acesso: TokenEmMemoria,
}

struct Conta {
    email: String,
    chave: ChavePrivada,
    kid: Option<String>,
    projeto: Option<String>,
}

/// O JSON da conta de servico. O erro nunca cita o conteudo: e ele que carrega a chave.
fn conta(raiz: &Path) -> Result<Conta, String> {
    let texto = base(raiz, &GCP_CONTA)?;
    let v: Value = serde_json::from_str(texto.expose())
        .map_err(|_| "o JSON da conta de servico do GCP nao e JSON valido".to_string())?;
    if v["type"].as_str() != Some("service_account") {
        return Err("o JSON guardado nao e de conta de servico (type service_account)".into());
    }
    let email = v["client_email"]
        .as_str()
        .filter(|e| !e.is_empty())
        .ok_or("conta de servico sem client_email")?
        .to_string();
    let pem = v["private_key"]
        .as_str()
        .ok_or("conta de servico sem private_key")?;
    let chave = ChavePrivada::de_pem(pem).map_err(|e| format!("private_key: {e}"))?;
    Ok(Conta {
        email,
        chave,
        kid: v["private_key_id"].as_str().map(str::to_string),
        projeto: v["project_id"].as_str().map(str::to_string),
    })
}

impl Gcp {
    pub fn novo(raiz: &Path, g: &CfgGcp, cfg: &ConfigCofres) -> Result<Self, String> {
        let url = g
            .url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| URL_PADRAO.into());
        let token_url = g
            .token_url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| TOKEN_PADRAO.into());
        Ok(Self {
            raiz: raiz.to_path_buf(),
            saida_api: Saida::nova(&url, cfg)?,
            saida_token: Saida::nova(&token_url, cfg)?,
            url: url.trim().trim_end_matches('/').to_string(),
            token_url: token_url.trim().to_string(),
            projeto: g.projeto.clone().filter(|p| !p.trim().is_empty()),
            acesso: TokenEmMemoria::default(),
        })
    }

    async fn acesso(&self, c: &Conta) -> Result<SecretValue, String> {
        if let Some(t) = self.acesso.valido() {
            return Ok(t);
        }
        let agora = agora();
        let mut cabecalho = json!({"alg": "RS256", "typ": "JWT"});
        if let Some(k) = &c.kid {
            cabecalho["kid"] = Value::String(k.clone());
        }
        let carga = json!({
            "iss": c.email,
            "scope": ESCOPO,
            "aud": self.token_url,
            "iat": agora,
            "exp": agora + VALIDADE_SEG,
        });
        let assercao = SecretValue::new(assinar_rs256(
            &serde_json::to_vec(&cabecalho).map_err(|e| e.to_string())?,
            &serde_json::to_vec(&carga).map_err(|e| e.to_string())?,
            &c.chave,
        )?);
        let cfg = ConfigOauth {
            token: self.token_url.clone(),
            ..Default::default()
        };
        let r = oauth::pedir_token_por(
            &cfg,
            vec![
                ("grant_type".into(), CONCESSAO.into()),
                ("assertion".into(), assercao.expose().to_string()),
            ],
            None,
            |spec| self.saida_token.enviar(spec),
        )
        .await
        .map_err(|e| limpar(format!("token do Google: {e}"), &[&assercao]))?;
        self.acesso.guardar(r.acesso.clone(), r.validade);
        Ok(r.acesso)
    }

    async fn ler_valor(&self, r: &ReferenciaExterna) -> Result<SecretValue, String> {
        let c = conta(&self.raiz)?;
        let projeto = self
            .projeto
            .clone()
            .or_else(|| c.projeto.clone())
            .ok_or("sem projeto: defina cofres.gcp.projeto (a conta nao traz project_id)")?;
        let url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/{}:access",
            self.url,
            segmento(&projeto, "projeto")?,
            segmento(&r.caminho, "nome")?,
            segmento(r.versao.as_deref().unwrap_or("latest"), "versao")?
        );
        for tentativa in 0..2 {
            let acesso = self.acesso(&c).await?;
            let mut spec = HttpRequestSpec::get(url.clone());
            spec.headers.insert(
                "Authorization".into(),
                format!("Bearer {}", acesso.expose()),
            );
            let resp = self.saida_api.enviar(spec).await;
            let resp = resp.map_err(|e| limpar(e, &[&acesso]))?;
            if resp.status == 401 && tentativa == 0 {
                self.acesso.esquecer();
                continue;
            }
            let v = json_de(&resp);
            if resp.status != 200 {
                let codigo = v["error"]["status"].as_str().map(str::to_string);
                return Err(motivo_do_status(resp.status, codigo));
            }
            let dados = v["payload"]["data"]
                .as_str()
                .ok_or("o Secret Manager respondeu sem payload.data")?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(dados)
                .map_err(|_| "payload.data sem base64 valido".to_string())?;
            let texto = SecretValue::new(
                String::from_utf8(bytes).map_err(|_| "o segredo nao e texto UTF-8".to_string())?,
            );
            return escolher_campo(texto.expose(), r.campo.as_deref());
        }
        Err("o Secret Manager recusou o acesso de novo depois de renovar (401)".into())
    }
}

impl CofreExterno for Gcp {
    fn tipo(&self) -> &'static str {
        "gcp"
    }

    fn ler<'a>(&'a self, r: &'a ReferenciaExterna) -> BoxFut<'a, Result<SecretValue, String>> {
        Box::pin(self.ler_valor(r))
    }
}
