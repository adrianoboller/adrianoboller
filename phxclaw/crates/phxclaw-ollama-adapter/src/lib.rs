use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub timeout_ms: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434/api/".into(),
            timeout_ms: 120_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("invalid Ollama base URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Ollama HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Ollama API returned status {status}: {body}")]
    Api { status: u16, body: String },
}

#[derive(Clone)]
pub struct OllamaClient {
    base: Url,
    http: Client,
}

impl OllamaClient {
    pub fn new(config: OllamaConfig) -> Result<Self, OllamaError> {
        let base = Url::parse(&config.base_url)?;
        let http = Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()?;
        Ok(Self { base, http })
    }

    pub async fn version(&self) -> Result<Value, OllamaError> {
        self.get_json("version").await
    }

    pub async fn list_models(&self) -> Result<Value, OllamaError> {
        self.get_json("tags").await
    }

    pub async fn running_models(&self) -> Result<Value, OllamaError> {
        self.get_json("ps").await
    }

    pub async fn generate(
        &self,
        model: &str,
        prompt: &str,
        system: Option<&str>,
        options: Option<Value>,
    ) -> Result<Value, OllamaError> {
        let mut body = json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        });
        if let Some(system) = system {
            body["system"] = Value::String(system.to_owned());
        }
        if let Some(options) = options {
            body["options"] = options;
        }
        self.post_json("generate", body).await
    }

    pub async fn chat(
        &self,
        model: &str,
        messages: &[OllamaMessage],
        tools: Option<Value>,
        think: Option<Value>,
        format: Option<Value>,
        options: Option<Value>,
    ) -> Result<Value, OllamaError> {
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false
        });
        if let Some(tools) = tools {
            body["tools"] = tools;
        }
        if let Some(think) = think {
            body["think"] = think;
        }
        if let Some(format) = format {
            body["format"] = format;
        }
        // Sem options nao ha como limitar a geracao: medido, um modelo pequeno a 0,8 de
        // temperatura as vezes nao para e so solta o provider no timeout (2 min).
        if let Some(options) = options {
            body["options"] = options;
        }
        self.post_json("chat", body).await
    }

    pub async fn embed(&self, model: &str, input: Value) -> Result<Value, OllamaError> {
        self.post_json(
            "embed",
            json!({
                "model": model,
                "input": input
            }),
        )
        .await
    }

    pub async fn pull(&self, model: &str) -> Result<Value, OllamaError> {
        self.post_json("pull", json!({"model": model, "stream": false}))
            .await
    }

    pub async fn show(&self, model: &str) -> Result<Value, OllamaError> {
        self.post_json("show", json!({"model": model})).await
    }

    pub async fn delete(&self, model: &str) -> Result<Value, OllamaError> {
        let url = self.endpoint("delete")?;
        let response = self
            .http
            .delete(url)
            .json(&json!({"model": model}))
            .send()
            .await?;
        parse_response(response).await
    }

    async fn get_json(&self, path: &str) -> Result<Value, OllamaError> {
        let response = self.http.get(self.endpoint(path)?).send().await?;
        parse_response(response).await
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value, OllamaError> {
        let response = self
            .http
            .post(self.endpoint(path)?)
            .json(&body)
            .send()
            .await?;
        parse_response(response).await
    }

    fn endpoint(&self, path: &str) -> Result<Url, OllamaError> {
        Ok(self.base.join(path)?)
    }
}

async fn parse_response(response: reqwest::Response) -> Result<Value, OllamaError> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(OllamaError::Api {
            status: status.as_u16(),
            body: text,
        });
    }
    if text.trim().is_empty() {
        return Ok(json!({"status":"ok"}));
    }
    Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!({"raw": text})))
}
