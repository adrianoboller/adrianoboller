use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, redirect::Policy, Method, Proxy};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::{Duration, Instant}};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpAuth {
    pub bearer: Option<String>,
    pub basic_username: Option<String>,
    pub basic_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartFile {
    pub field_name: String,
    pub file_name: String,
    pub content_type: Option<String>,
    pub bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestSpec {
    pub uuid: Uuid,
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub query: Vec<(String, String)>,
    pub body_base64: Option<String>,
    pub json: Option<Value>,
    pub form: Vec<(String, String)>,
    pub multipart_fields: Vec<MultipartField>,
    pub multipart_files: Vec<MultipartFile>,
    pub auth: HttpAuth,
    pub timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub follow_redirects: bool,
    pub max_redirects: usize,
    pub proxy: Option<String>,
    pub user_agent: Option<String>,
    pub accept_invalid_certs: bool,
}

impl HttpRequestSpec {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            uuid: new_uuid_v7(),
            method: "GET".into(),
            url: url.into(),
            headers: BTreeMap::new(),
            query: Vec::new(),
            body_base64: None,
            json: None,
            form: Vec::new(),
            multipart_fields: Vec::new(),
            multipart_files: Vec::new(),
            auth: HttpAuth::default(),
            timeout_ms: 30_000,
            connect_timeout_ms: 10_000,
            follow_redirects: true,
            max_redirects: 10,
            proxy: None,
            user_agent: Some("PhxClaw/0.8".into()),
            accept_invalid_certs: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResult {
    pub request_uuid: Uuid,
    pub final_url: String,
    pub status: u16,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body_base64: String,
    pub content_type: Option<String>,
    pub elapsed_ms: u128,
    pub received_at: DateTime<Utc>,
}

impl HttpResult {
    pub fn bytes(&self) -> Result<Vec<u8>, HttpClientError> {
        BASE64.decode(&self.body_base64).map_err(|e| HttpClientError::Decode(e.to_string()))
    }

    pub fn text(&self) -> Result<String, HttpClientError> {
        String::from_utf8(self.bytes()?).map_err(|e| HttpClientError::Decode(e.to_string()))
    }

    pub fn json(&self) -> Result<Value, HttpClientError> {
        Ok(serde_json::from_slice(&self.bytes()?)?)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpGetResultMode {
    BytesBase64,
    Text,
    Json,
    Headers,
    Status,
    Metadata,
}

#[derive(Debug, Error)]
pub enum HttpClientError {
    #[error("invalid HTTP method: {0}")]
    InvalidMethod(String),
    #[error("invalid header {0}: {1}")]
    InvalidHeader(String, String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTP error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("decode error: {0}")]
    Decode(String),
}

/// PhxClaw equivalent of a complete HTTPRequest operation.
pub async fn http_request(spec: &HttpRequestSpec) -> Result<HttpResult, HttpClientError> {
    let method = Method::from_bytes(spec.method.as_bytes())
        .map_err(|_| HttpClientError::InvalidMethod(spec.method.clone()))?;

    let redirect = if spec.follow_redirects {
        Policy::limited(spec.max_redirects)
    } else {
        Policy::none()
    };

    let mut builder = reqwest::Client::builder()
        .redirect(redirect)
        .timeout(Duration::from_millis(spec.timeout_ms))
        .connect_timeout(Duration::from_millis(spec.connect_timeout_ms))
        .danger_accept_invalid_certs(spec.accept_invalid_certs)
        .cookie_store(true);

    if let Some(proxy) = &spec.proxy {
        builder = builder.proxy(Proxy::all(proxy).map_err(HttpClientError::Request)?);
    }
    if let Some(ua) = &spec.user_agent {
        builder = builder.user_agent(ua.clone());
    }

    let client = builder.build()?;
    let mut request = client.request(method, &spec.url).query(&spec.query);

    let mut headers = HeaderMap::new();
    for (name, value) in &spec.headers {
        let hname = HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| HttpClientError::InvalidHeader(name.clone(), e.to_string()))?;
        let hvalue = HeaderValue::from_str(value)
            .map_err(|e| HttpClientError::InvalidHeader(name.clone(), e.to_string()))?;
        headers.insert(hname, hvalue);
    }
    request = request.headers(headers);

    if let Some(token) = &spec.auth.bearer {
        request = request.bearer_auth(token);
    }
    if let Some(username) = &spec.auth.basic_username {
        request = request.basic_auth(username, spec.auth.basic_password.clone());
    }

    let body_modes = [
        spec.body_base64.is_some(),
        spec.json.is_some(),
        !spec.form.is_empty(),
        !spec.multipart_fields.is_empty() || !spec.multipart_files.is_empty(),
    ].into_iter().filter(|v| *v).count();
    if body_modes > 1 {
        return Err(HttpClientError::InvalidRequest(
            "body_base64, json, form and multipart are mutually exclusive".into(),
        ));
    }

    if let Some(body_b64) = &spec.body_base64 {
        let body = BASE64.decode(body_b64).map_err(|e| HttpClientError::Decode(e.to_string()))?;
        request = request.body(body);
    } else if let Some(value) = &spec.json {
        request = request.json(value);
    } else if !spec.form.is_empty() {
        request = request.form(&spec.form);
    } else if !spec.multipart_fields.is_empty() || !spec.multipart_files.is_empty() {
        let mut form = reqwest::multipart::Form::new();
        for field in &spec.multipart_fields {
            form = form.text(field.name.clone(), field.value.clone());
        }
        for file in &spec.multipart_files {
            let bytes = BASE64.decode(&file.bytes_base64)
                .map_err(|e| HttpClientError::Decode(e.to_string()))?;
            let mut part = reqwest::multipart::Part::bytes(bytes).file_name(file.file_name.clone());
            if let Some(content_type) = &file.content_type {
                part = part.mime_str(content_type)
                    .map_err(|e| HttpClientError::InvalidRequest(e.to_string()))?;
            }
            form = form.part(file.field_name.clone(), part);
        }
        request = request.multipart(form);
    }

    let started = Instant::now();
    let response = request.send().await?;
    let final_url = response.url().to_string();
    let status = response.status().as_u16();
    let content_type = response.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).map(ToOwned::to_owned);
    let response_headers = headers_to_map(response.headers());
    let bytes = response.bytes().await?;

    Ok(HttpResult {
        request_uuid: spec.uuid,
        final_url,
        status,
        headers: response_headers,
        body_base64: BASE64.encode(bytes),
        content_type,
        elapsed_ms: started.elapsed().as_millis(),
        received_at: Utc::now(),
    })
}

/// PhxClaw equivalent of HTTPGetResult over a previously completed request.
pub fn http_get_result(result: &HttpResult, mode: HttpGetResultMode) -> Result<Value, HttpClientError> {
    Ok(match mode {
        HttpGetResultMode::BytesBase64 => Value::String(result.body_base64.clone()),
        HttpGetResultMode::Text => Value::String(result.text()?),
        HttpGetResultMode::Json => result.json()?,
        HttpGetResultMode::Headers => serde_json::to_value(&result.headers)?,
        HttpGetResultMode::Status => json!(result.status),
        HttpGetResultMode::Metadata => json!({
            "request_uuid": result.request_uuid,
            "final_url": result.final_url,
            "status": result.status,
            "content_type": result.content_type,
            "elapsed_ms": result.elapsed_ms,
            "received_at": result.received_at,
        }),
    })
}

fn headers_to_map(headers: &HeaderMap) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::<String, Vec<String>>::new();
    for (name, value) in headers {
        out.entry(name.as_str().to_ascii_lowercase())
            .or_default()
            .push(value.to_str().unwrap_or_default().to_owned());
    }
    out
}
