use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use url::Url;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum OllamaError {
    #[error("origin is not allowed")]
    OriginDenied,
    #[error("model pull requires explicit approval")]
    PullApprovalRequired,
    #[error("invalid model name")]
    InvalidModel,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OllamaConfig {
    pub base_url: String,
    pub allowed_remote_origins: Vec<String>,
    pub allow_remote_https: bool,
    pub pull_requires_approval: bool,
    pub cloud_api_key_secret_handle: Option<String>,
}
impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".into(),
            allowed_remote_origins: vec![],
            allow_remote_https: false,
            pull_requires_approval: true,
            cloud_api_key_secret_handle: None,
        }
    }
}

pub fn validate_origin(cfg: &OllamaConfig) -> Result<Url, OllamaError> {
    let u = Url::parse(&cfg.base_url).map_err(|_| OllamaError::OriginDenied)?;
    if !u.username().is_empty() || u.password().is_some() {
        return Err(OllamaError::OriginDenied);
    }
    let host = u.host_str().unwrap_or_default();
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1");
    if loopback && matches!(u.scheme(), "http" | "https") {
        return Ok(u);
    }
    if u.scheme() == "https"
        && cfg.allow_remote_https
        && cfg
            .allowed_remote_origins
            .iter()
            .any(|o| o == &u.origin().ascii_serialization())
    {
        return Ok(u);
    }
    Err(OllamaError::OriginDenied)
}
fn model_ok(m: &str) -> bool {
    !m.trim().is_empty()
        && m.len() <= 200
        && m.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:/".contains(c))
}
pub fn native_chat_request(
    model: &str,
    messages: Value,
    tools: Option<Value>,
    think: Option<&str>,
) -> Result<Value, OllamaError> {
    if !model_ok(model) {
        return Err(OllamaError::InvalidModel);
    }
    let mut v = json!({"model":model,"messages":messages,"stream":false});
    if let Some(t) = tools {
        v["tools"] = t;
    }
    if let Some(t) = think {
        v["think"] = json!(t);
    }
    Ok(v)
}
pub fn embed_request(model: &str, input: Value) -> Result<Value, OllamaError> {
    if !model_ok(model) {
        return Err(OllamaError::InvalidModel);
    }
    Ok(json!({"model":model,"input":input}))
}
pub fn endpoints() -> [(&'static str, &'static str); 7] {
    [
        ("health", "/api/version"),
        ("models", "/api/tags"),
        ("show", "/api/show"),
        ("chat", "/api/chat"),
        ("embed", "/api/embed"),
        ("openai", "/v1/chat/completions"),
        ("anthropic", "/v1/messages"),
    ]
}
pub fn anthropic_base_path() -> &'static str {
    "/"
}
pub fn plan_pull(model: &str, approved: bool, cfg: &OllamaConfig) -> Result<Value, OllamaError> {
    if !model_ok(model) {
        return Err(OllamaError::InvalidModel);
    }
    if cfg.pull_requires_approval && !approved {
        return Err(OllamaError::PullApprovalRequired);
    }
    Ok(json!({"endpoint":"/api/pull","body":{"model":model,"stream":false}}))
}
