use phxclaw_http_client::{http_request, HttpClientError, HttpRequestSpec, HttpResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressPolicy {
    pub enabled: bool,
    /// Exact origins: scheme://host[:port], e.g. http://127.0.0.1:11434.
    pub allowed_origins: BTreeSet<String>,
    pub allow_http: bool,
}

impl Default for EgressPolicy {
    fn default() -> Self {
        Self { enabled: false, allowed_origins: BTreeSet::new(), allow_http: false }
    }
}

#[derive(Debug, Error)]
pub enum EgressError {
    #[error("network egress is disabled")]
    Disabled,
    #[error("invalid target URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("HTTP is denied for target origin: {0}")]
    InsecureHttp(String),
    #[error("target origin is not allowlisted: {0}")]
    OriginDenied(String),
    #[error(transparent)]
    Http(#[from] HttpClientError),
}

#[derive(Clone)]
pub struct EgressBroker {
    policy: EgressPolicy,
}

impl EgressBroker {
    pub fn new(policy: EgressPolicy) -> Self { Self { policy } }

    pub fn validate_url(&self, raw: &str) -> Result<Url, EgressError> {
        if !self.policy.enabled { return Err(EgressError::Disabled); }
        let url = Url::parse(raw)?;
        let scheme = url.scheme();
        if scheme != "https" && scheme != "http" {
            return Err(EgressError::OriginDenied(origin(&url)));
        }
        let target = origin(&url);
        if scheme == "http" && !self.policy.allow_http {
            return Err(EgressError::InsecureHttp(target));
        }
        if !self.policy.allowed_origins.contains(&target) {
            return Err(EgressError::OriginDenied(target));
        }
        Ok(url)
    }

    pub async fn request(&self, spec: &HttpRequestSpec) -> Result<HttpResult, EgressError> {
        self.validate_url(&spec.url)?;
        if let Some(proxy) = &spec.proxy { self.validate_url(proxy)?; }
        Ok(http_request(spec).await?)
    }
}

fn origin(url: &Url) -> String {
    let host = url.host_str().unwrap_or("");
    match url.port() {
        Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
        None => format!("{}://{}", url.scheme(), host),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_by_default() {
        let broker = EgressBroker::new(EgressPolicy::default());
        assert!(matches!(broker.validate_url("https://example.com"), Err(EgressError::Disabled)));
    }

    #[test]
    fn exact_origin_allowlist() {
        let mut policy = EgressPolicy::default();
        policy.enabled = true;
        policy.allowed_origins.insert("https://api.example.com".into());
        let broker = EgressBroker::new(policy);
        assert!(broker.validate_url("https://api.example.com/v1/test").is_ok());
        assert!(broker.validate_url("https://example.com").is_err());
    }
}
