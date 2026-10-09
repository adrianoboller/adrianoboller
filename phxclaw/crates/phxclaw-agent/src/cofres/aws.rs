//! AWS Secrets Manager: `GetSecretValue` (JSON 1.1, `X-Amz-Target:
//! secretsmanager.GetSecretValue`) assinado com a SigV4 da casa (`sigv4.rs`). A credencial
//! base -- o access key ID da configuracao e o secret access key do broker local, mais o
//! token de sessao opcional -- so vai ao endpoint declarado, e so como assinatura: o
//! segredo da AWS nunca sai da maquina, so o HMAC dele.
//!
//! `versao`: um `VersionId` (o UUID que a AWS da a cada versao) ou `estagio:NOME` para um
//! `VersionStage` (`estagio:AWSPREVIOUS`). Sem ela, a AWS devolve o `AWSCURRENT`.

use super::sigv4;
use super::{
    AWS_SEGREDO, AWS_TOKEN_SESSAO, CfgAws, ConfigCofres, Saida, base, base_opcional,
    escolher_campo, json_de, limpar, motivo_do_status,
};
use base64::Engine;
use phxclaw_http_client::HttpRequestSpec;
use phxclaw_secret_broker::SecretValue;
use phxclaw_secret_broker::cofre::{BoxFut, CofreExterno, ReferenciaExterna};
use reqwest::Url;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const SERVICO: &str = "secretsmanager";
pub const ALVO: &str = "secretsmanager.GetSecretValue";
pub const TIPO_DO_CORPO: &str = "application/x-amz-json-1.1";

pub struct Aws {
    raiz: PathBuf,
    regiao: String,
    chave_id: String,
    url: Url,
    saida: Saida,
}

impl Aws {
    pub fn novo(raiz: &Path, a: &CfgAws, cfg: &ConfigCofres) -> Result<Self, String> {
        let regiao = a.regiao.trim().to_string();
        if regiao.is_empty()
            || !regiao
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!("cofres.aws.regiao invalida: {regiao:?}"));
        }
        if a.chave_id.trim().is_empty() {
            return Err("AWS sem cofres.aws.chave_id (o access key ID)".into());
        }
        let url = a
            .url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| format!("https://secretsmanager.{regiao}.amazonaws.com"));
        let saida = Saida::nova(&url, cfg)?;
        Ok(Self {
            raiz: raiz.to_path_buf(),
            regiao,
            chave_id: a.chave_id.trim().to_string(),
            url: Url::parse(url.trim()).map_err(|e| e.to_string())?,
            saida,
        })
    }

    async fn ler_valor(&self, r: &ReferenciaExterna) -> Result<SecretValue, String> {
        let segredo = base(&self.raiz, &AWS_SEGREDO)?;
        let sessao = base_opcional(&self.raiz, &AWS_TOKEN_SESSAO)?;
        let mut corpo = Map::new();
        corpo.insert("SecretId".into(), Value::String(r.caminho.clone()));
        match r.versao.as_deref() {
            Some(v) if v.starts_with("estagio:") => {
                corpo.insert("VersionStage".into(), Value::String(v[8..].to_string()));
            }
            Some(v) => {
                corpo.insert("VersionId".into(), Value::String(v.to_string()));
            }
            None => {}
        }
        let corpo = serde_json::to_vec(&Value::Object(corpo)).map_err(|e| e.to_string())?;
        let data_hora = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let host = match (self.url.host_str(), self.url.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_string(),
            _ => return Err("endpoint da AWS sem host".into()),
        };
        let mut cabecalhos = vec![
            ("Content-Type".to_string(), TIPO_DO_CORPO.to_string()),
            ("Host".to_string(), host),
            ("X-Amz-Date".to_string(), data_hora.clone()),
            ("X-Amz-Target".to_string(), ALVO.to_string()),
        ];
        if let Some(t) = &sessao {
            cabecalhos.push(("X-Amz-Security-Token".into(), t.expose().to_string()));
        }
        let autorizacao = sigv4::autorizacao(
            &sigv4::Pedido {
                metodo: "POST",
                caminho: self.url.path(),
                query: &[],
                cabecalhos: &cabecalhos,
                corpo: &corpo,
            },
            &self.chave_id,
            segredo.expose(),
            &self.regiao,
            SERVICO,
            &data_hora,
        )?;
        let mut spec = HttpRequestSpec::get(self.url.to_string());
        spec.method = "POST".into();
        for (k, v) in cabecalhos {
            // O `Host` o cliente poe pela URL (o mesmo texto que se assinou).
            if k != "Host" {
                spec.headers.insert(k, v);
            }
        }
        spec.headers.insert("Authorization".into(), autorizacao);
        spec.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&corpo));
        let mut segredos = vec![&segredo];
        if let Some(t) = &sessao {
            segredos.push(t);
        }
        let resp = self
            .saida
            .enviar(spec)
            .await
            .map_err(|e| limpar(e, &segredos))?;
        let v = json_de(&resp);
        if resp.status != 200 {
            // `__type` e o codigo do erro (`com.amazonaws...#ResourceNotFoundException` ou so
            // o nome); a `message` e texto livre e fica de fora.
            let tipo = v["__type"]
                .as_str()
                .map(|t| t.rsplit('#').next().unwrap_or(t).to_string());
            return Err(motivo_do_status(resp.status, tipo));
        }
        let texto = match (v["SecretString"].as_str(), v["SecretBinary"].as_str()) {
            (Some(t), _) => t.to_string(),
            (None, Some(b)) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b)
                    .map_err(|_| "SecretBinary sem base64 valido".to_string())?;
                String::from_utf8(bytes)
                    .map_err(|_| "SecretBinary nao e texto UTF-8".to_string())?
            }
            (None, None) => return Err("a AWS respondeu sem SecretString nem SecretBinary".into()),
        };
        escolher_campo(&texto, r.campo.as_deref())
    }
}

impl CofreExterno for Aws {
    fn tipo(&self) -> &'static str {
        "aws"
    }

    fn ler<'a>(&'a self, r: &'a ReferenciaExterna) -> BoxFut<'a, Result<SecretValue, String>> {
        Box::pin(self.ler_valor(r))
    }
}
