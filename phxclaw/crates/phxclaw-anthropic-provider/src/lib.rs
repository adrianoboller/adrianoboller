use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;
use uuid::Uuid;
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProviderError {
    #[error("origin denied")]
    OriginDenied,
    #[error("invalid model")]
    InvalidModel,
    #[error("invalid secret handle")]
    InvalidSecretHandle,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialRef {
    pub secret_uuid: Uuid,
    pub header: String,
    pub scheme: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HttpPlan {
    pub method: String,
    pub url: String,
    pub credential: CredentialRef,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}
fn model_ok(m: &str) -> bool {
    !m.trim().is_empty()
        && m.len() <= 200
        && m.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:/".contains(c))
}
fn fixed_url(
    origin: &str,
    path: &str,
    official: &str,
    allow_custom: &[String],
) -> Result<Url, ProviderError> {
    let u = Url::parse(origin).map_err(|_| ProviderError::OriginDenied)?;
    if u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() {
        return Err(ProviderError::OriginDenied);
    }
    let o = u.origin().ascii_serialization();
    if o != official && !allow_custom.iter().any(|x| x == &o) {
        return Err(ProviderError::OriginDenied);
    }
    u.join(path).map_err(|_| ProviderError::OriginDenied)
}

pub fn messages_plan(
    origin: &str,
    allow_custom: &[String],
    secret_uuid: Uuid,
    api_version: &str,
    model: &str,
    max_tokens: u32,
    messages: Value,
    tools: Option<Value>,
    output_config: Option<Value>,
) -> Result<HttpPlan, ProviderError> {
    if !model_ok(model) {
        return Err(ProviderError::InvalidModel);
    }
    let u = fixed_url(
        origin,
        "/v1/messages",
        "https://api.anthropic.com",
        allow_custom,
    )?;
    let mut body = serde_json::json!({"model":model,"max_tokens":max_tokens,"messages":messages});
    if let Some(t) = tools {
        body["tools"] = t;
    }
    if let Some(o) = output_config {
        body["output_config"] = o;
    }
    Ok(HttpPlan {
        method: "POST".into(),
        url: u.to_string(),
        credential: CredentialRef {
            secret_uuid,
            header: "x-api-key".into(),
            scheme: None,
        },
        headers: vec![
            ("Content-Type".into(), "application/json".into()),
            ("anthropic-version".into(), api_version.into()),
        ],
        body,
    })
}
