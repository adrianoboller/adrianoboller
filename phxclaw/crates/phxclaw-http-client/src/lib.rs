use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use reqwest::{
    Method, Proxy,
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
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
        BASE64
            .decode(&self.body_base64)
            .map_err(|e| HttpClientError::Decode(e.to_string()))
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
    #[error("response body exceeds the limit of {0} bytes")]
    BodyTooLarge(usize),
}

/// O que um pedido precisa e o `HttpRequestSpec` nao carrega, porque nao e dado do pedido e
/// sim politica de quem o faz: o teto do corpo e o endereco ja conferido.
#[derive(Debug, Clone, Default)]
pub struct HttpOptions {
    /// Teto do corpo da resposta. Lido em pedacos e abortado ao passar: ler tudo e conferir
    /// depois e justamente a alocacao que o teto existe para impedir.
    pub max_body_bytes: Option<usize>,
    /// Endereco fixo por host. Quem confere o destino pelo IP (SSRF) fixa aqui o IP que
    /// conferiu: sem isso o cliente resolveria o nome de novo, e um DNS que responde publico
    /// na conferencia e 127.0.0.1 na conexao passaria pela guarda (DNS rebinding).
    ///
    /// Endereco fixado quer dizer conexao DIRETA: o cliente desliga o proxy do ambiente
    /// (`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`). Pelo proxy, quem resolve o nome e o proxy, e
    /// o IP fixado seria ignorado sem erro nenhum. Quem precisa do proxy confere antes com
    /// `proxy_do_ambiente` e decide (e diz) que abre mao da fixacao, mandando `resolve` vazio.
    pub resolve: Vec<(String, Vec<SocketAddr>)>,
    /// Soma os bytes de corpo lidos. Quem segue redirecionamento salto a salto paga o corpo
    /// de CADA salto, nao so o do ultimo: sem isso o teto de um no valia ate 6x (5 saltos).
    pub contador: Option<Arc<AtomicUsize>>,
}

/// O nome da variavel de proxy do ambiente que o cliente usaria para `url` (nunca o valor:
/// a URL do proxy pode levar usuario e senha), ou `None` se nenhuma vale para ela -- inclusive
/// quando o `NO_PROXY` a exclui. Mesma ordem do reqwest: a do esquema (`HTTPS_PROXY` ou
/// `HTTP_PROXY`, maiuscula antes de minuscula), depois `ALL_PROXY`.
pub fn proxy_do_ambiente(url: &url::Url) -> Option<&'static str> {
    proxy_do_ambiente_com(url, |k| std::env::var(k).ok())
}

/// `proxy_do_ambiente` com o ambiente nas maos de quem chama: o teste nao pode mexer no
/// ambiente do processo, que os outros testes em paralelo leem.
pub fn proxy_do_ambiente_com(
    url: &url::Url,
    var: impl Fn(&str) -> Option<String>,
) -> Option<&'static str> {
    let tem = |k: &str| var(k).is_some_and(|v| !v.trim().is_empty());
    let candidatas: &[&'static str] = match url.scheme() {
        "https" => &["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"],
        "http" => &["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"],
        _ => &[],
    };
    let achada = candidatas.iter().copied().find(|k| tem(k))?;
    let host = url
        .host_str()?
        .trim_start_matches('[')
        .trim_end_matches(']');
    let excluida = ["NO_PROXY", "no_proxy"]
        .iter()
        .find_map(|k| var(k).filter(|v| !v.trim().is_empty()))
        .is_some_and(|lista| no_proxy_exclui(&lista, host));
    (!excluida).then_some(achada)
}

/// A regra do `NO_PROXY` do reqwest: `*` exclui tudo; IP ou bloco CIDR confere o host IP;
/// dominio confere o proprio nome e os subdominios (com ou sem o ponto na frente).
fn no_proxy_exclui(lista: &str, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let ip_do_host: Option<IpAddr> = host.parse().ok();
    lista
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .any(|e| {
            let e = e.to_ascii_lowercase();
            if e == "*" {
                return true;
            }
            if let Some(ip) = ip_do_host {
                return match e.split_once('/') {
                    Some((rede, bits)) => match (rede.parse::<IpAddr>(), bits.parse::<u32>()) {
                        (Ok(rede), Ok(bits)) => na_rede(ip, rede, bits),
                        _ => false,
                    },
                    None => e.parse::<IpAddr>().is_ok_and(|x| x == ip),
                };
            }
            let d = e.trim_start_matches("*.").trim_start_matches('.');
            host == d || host.ends_with(&format!(".{d}"))
        })
}

fn na_rede(ip: IpAddr, rede: IpAddr, bits: u32) -> bool {
    match (ip, rede) {
        (IpAddr::V4(a), IpAddr::V4(b)) if bits <= 32 => {
            let m = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            u32::from(a) & m == u32::from(b) & m
        }
        (IpAddr::V6(a), IpAddr::V6(b)) if bits <= 128 => {
            let m = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - bits)
            };
            u128::from(a) & m == u128::from(b) & m
        }
        _ => false,
    }
}

/// PhxClaw equivalent of a complete HTTPRequest operation.
pub async fn http_request(spec: &HttpRequestSpec) -> Result<HttpResult, HttpClientError> {
    http_request_with(spec, &HttpOptions::default()).await
}

/// O mesmo pedido, com o teto do corpo e o endereco fixado de `opts`. Um motor so: o
/// `http_request` e este com as opcoes vazias, para a montagem do pedido nao ter duas copias.
pub async fn http_request_with(
    spec: &HttpRequestSpec,
    opts: &HttpOptions,
) -> Result<HttpResult, HttpClientError> {
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

    if !opts.resolve.is_empty() {
        if spec.proxy.is_some() {
            return Err(HttpClientError::InvalidRequest(
                "endereco fixado e proxy no mesmo pedido: pelo proxy o IP fixado nao vale".into(),
            ));
        }
        // O reqwest liga o proxy do ambiente sozinho e, por ele, o IP fixado e ignorado em
        // silencio: o proxy resolve o nome de novo e o DNS rebinding volta.
        builder = builder.no_proxy();
    }
    if let Some(proxy) = &spec.proxy {
        builder = builder.proxy(Proxy::all(proxy).map_err(HttpClientError::Request)?);
    }
    if let Some(ua) = &spec.user_agent {
        builder = builder.user_agent(ua.clone());
    }
    for (host, enderecos) in &opts.resolve {
        builder = builder.resolve_to_addrs(host, enderecos);
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
    ]
    .into_iter()
    .filter(|v| *v)
    .count();
    if body_modes > 1 {
        return Err(HttpClientError::InvalidRequest(
            "body_base64, json, form and multipart are mutually exclusive".into(),
        ));
    }

    if let Some(body_b64) = &spec.body_base64 {
        let body = BASE64
            .decode(body_b64)
            .map_err(|e| HttpClientError::Decode(e.to_string()))?;
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
            let bytes = BASE64
                .decode(&file.bytes_base64)
                .map_err(|e| HttpClientError::Decode(e.to_string()))?;
            let mut part = reqwest::multipart::Part::bytes(bytes).file_name(file.file_name.clone());
            if let Some(content_type) = &file.content_type {
                part = part
                    .mime_str(content_type)
                    .map_err(|e| HttpClientError::InvalidRequest(e.to_string()))?;
            }
            form = form.part(file.field_name.clone(), part);
        }
        request = request.multipart(form);
    }

    let started = Instant::now();
    let mut response = request.send().await?;
    let final_url = response.url().to_string();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);
    let response_headers = headers_to_map(response.headers());
    let bytes = match opts.max_body_bytes {
        None => response.bytes().await?.to_vec(),
        Some(teto) => {
            // O Content-Length declarado ja recusa sem ler; o laco cobre o servidor que nao
            // declara (chunked) ou que declara menos do que manda.
            if response.content_length().is_some_and(|n| n > teto as u64) {
                return Err(HttpClientError::BodyTooLarge(teto));
            }
            let mut corpo = Vec::new();
            while let Some(pedaco) = response.chunk().await? {
                if corpo.len() + pedaco.len() > teto {
                    return Err(HttpClientError::BodyTooLarge(teto));
                }
                corpo.extend_from_slice(&pedaco);
            }
            corpo
        }
    };
    if let Some(c) = &opts.contador {
        c.fetch_add(bytes.len(), Ordering::SeqCst);
    }

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
pub fn http_get_result(
    result: &HttpResult,
    mode: HttpGetResultMode,
) -> Result<Value, HttpClientError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Servidor HTTP minimo: responde `corpo` a cada conexao e conta quantas recebeu. Serve
    /// de alvo e de proxy falso (o proxy de `http://` recebe o pedido com a URL inteira).
    fn servidor(corpo: &'static str) -> (SocketAddr, Arc<AtomicUsize>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let conexoes = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&conexoes);
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                c.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{corpo}",
                    corpo.len()
                );
            }
        });
        (addr, conexoes)
    }

    #[test]
    fn proxy_do_ambiente_segue_o_esquema_e_o_no_proxy() {
        let amb = |pares: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pares
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let u = |s: &str| url::Url::parse(s).unwrap();
        let so_https = amb(&[("HTTPS_PROXY", "http://p:1")]);
        assert_eq!(
            proxy_do_ambiente_com(&u("https://a.example/x"), so_https),
            Some("HTTPS_PROXY")
        );
        assert_eq!(
            proxy_do_ambiente_com(&u("http://a.example/x"), so_https),
            None
        );
        let tudo = amb(&[
            ("all_proxy", "http://p:1"),
            ("NO_PROXY", "localhost, .interno.example,10.0.0.0/8,::1"),
        ]);
        assert_eq!(
            proxy_do_ambiente_com(&u("http://a.example/"), tudo),
            Some("all_proxy")
        );
        for fora in [
            "http://localhost:8/",
            "http://x.interno.example/",
            "http://interno.example/",
            "http://10.2.3.4/",
            "http://[::1]:9/",
        ] {
            assert_eq!(proxy_do_ambiente_com(&u(fora), tudo), None, "{fora}");
        }
        assert_eq!(
            proxy_do_ambiente_com(&u("http://11.0.0.1/"), tudo),
            Some("all_proxy")
        );
        let vazio = amb(&[("HTTPS_PROXY", "  ")]);
        assert_eq!(proxy_do_ambiente_com(&u("https://a/"), vazio), None);
    }

    /// O lado filho de `ip_fixado_nao_passa_pelo_proxy_do_ambiente`: roda em processo
    /// proprio, com o `HTTP_PROXY` que o pai armou (mexer no ambiente deste processo mudaria
    /// o de todos os testes em paralelo). Sozinho, sem o pai, nao faz nada.
    #[tokio::test]
    #[ignore = "processo filho de ip_fixado_nao_passa_pelo_proxy_do_ambiente"]
    async fn filho_do_proxy_do_ambiente() {
        let Ok(alvo) = std::env::var("PHX_TESTE_ALVO") else {
            return;
        };
        let alvo: SocketAddr = alvo.parse().unwrap();
        let url = format!("http://fixado.invalid:{}/x", alvo.port());
        let opts = HttpOptions {
            resolve: vec![("fixado.invalid".into(), vec![alvo])],
            ..HttpOptions::default()
        };
        let texto = |r: Result<HttpResult, HttpClientError>| match r {
            Ok(r) => r.text().unwrap_or_default(),
            Err(e) => format!("erro {e}"),
        };
        println!(
            "FIXADO={}",
            texto(http_request_with(&HttpRequestSpec::get(&url), &opts).await)
        );
        println!(
            "SEM_FIXAR={}",
            texto(http_request(&HttpRequestSpec::get(&url)).await)
        );
    }

    /// Com IP fixado, o pedido vai DIRETO ao IP conferido mesmo com `HTTP_PROXY` no ambiente;
    /// sem fixar, o proxy do ambiente vale (e a prova de que o filho tinha o proxy armado).
    ///
    /// RED medido: em `http_request_with`, o `builder.no_proxy()` retirado (`// REPOSTO`):
    /// o pedido fixado vai ao proxy falso (`FIXADO=proxy`, 2 conexoes no proxy).
    #[test]
    fn ip_fixado_nao_passa_pelo_proxy_do_ambiente() {
        let (alvo, toques_alvo) = servidor("alvo");
        let (proxy, toques_proxy) = servidor("proxy");
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "tests::filho_do_proxy_do_ambiente",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]);
        for k in [
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "NO_PROXY",
            "no_proxy",
        ] {
            cmd.env_remove(k);
        }
        let saida = cmd
            .env("HTTP_PROXY", format!("http://{proxy}"))
            .env("http_proxy", format!("http://{proxy}"))
            .env("PHX_TESTE_ALVO", alvo.to_string())
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&saida.stdout);
        assert!(
            saida.status.success(),
            "{out}\n{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        assert!(
            out.contains("FIXADO=alvo"),
            "o IP fixado foi pelo proxy: {out}"
        );
        assert!(
            out.contains("SEM_FIXAR=proxy"),
            "o proxy do filho nao valia: {out}"
        );
        assert_eq!(toques_proxy.load(Ordering::SeqCst), 1, "{out}");
        assert_eq!(toques_alvo.load(Ordering::SeqCst), 1, "{out}");
    }

    #[test]
    fn ip_fixado_com_proxy_explicito_e_recusado() {
        let mut spec = HttpRequestSpec::get("http://a.invalid/");
        spec.proxy = Some("http://127.0.0.1:1".into());
        let opts = HttpOptions {
            resolve: vec![("a.invalid".into(), vec!["127.0.0.1:1".parse().unwrap()])],
            ..HttpOptions::default()
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let r = rt.block_on(http_request_with(&spec, &opts));
        assert!(
            matches!(r, Err(HttpClientError::InvalidRequest(_))),
            "{r:?}"
        );
    }
}
