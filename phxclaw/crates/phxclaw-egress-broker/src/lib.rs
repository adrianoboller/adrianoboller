use phxclaw_http_client::{
    HttpClientError, HttpOptions, HttpRequestSpec, HttpResult, http_request_with,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EgressPolicy {
    pub enabled: bool,
    /// Exact origins: scheme://host[:port], e.g. http://127.0.0.1:11434.
    pub allowed_origins: BTreeSet<String>,
    pub allow_http: bool,
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
    #[error("accept_invalid_certs is denied by egress policy")]
    InvalidCertsDenied,
    #[error("too many redirects (limit {0})")]
    TooManyRedirects(usize),
    /// Recusa de quem confere o destino fora da lista de origens (a guarda de IP do no
    /// HTTP do fluxo), com o motivo dela.
    #[error("destination refused: {0}")]
    Refused(String),
}

#[derive(Clone)]
pub struct EgressBroker {
    policy: EgressPolicy,
}

impl EgressBroker {
    pub fn new(policy: EgressPolicy) -> Self {
        Self { policy }
    }

    pub fn validate_url(&self, raw: &str) -> Result<Url, EgressError> {
        if !self.policy.enabled {
            return Err(EgressError::Disabled);
        }
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

    /// O broker segue os redirects ELE MESMO, salto a salto, reconferindo cada Location
    /// contra a lista. Deixar o cliente HTTP seguir sozinho validava so a primeira URL: uma
    /// origem permitida respondendo 302 para 169.254.169.254 ou 127.0.0.1 levava o pedido
    /// para dentro da maquina (SSRF).
    pub async fn request(&self, spec: &HttpRequestSpec) -> Result<HttpResult, EgressError> {
        // Certificado invalido aceito e MITM consentido: a politica de egresso nao delega.
        if spec.accept_invalid_certs {
            return Err(EgressError::InvalidCertsDenied);
        }
        self.validate_url(&spec.url)?;
        if let Some(proxy) = &spec.proxy {
            self.validate_url(proxy)?;
        }
        request_checked(spec, |url| {
            let r = self
                .validate_url(url.as_str())
                .map(|_| HttpOptions::default());
            async move { r }
        })
        .await
    }
}

/// O laco de redirecionamento, com a conferencia de cada salto nas maos de quem chama: a
/// lista de origens do `EgressBroker` e uma; a guarda de IP do no HTTP do fluxo e outra. As
/// duas pedem o MESMO laco -- salto a salto, credencial que nao atravessa origem, POST que
/// vira GET no 303 --, e um segundo laco seria a copia que alguem esquece de consertar.
///
/// `check` recebe cada destino, o primeiro inclusive, ANTES de qualquer conexao a ele, e
/// devolve as opcoes do pedido (teto do corpo, endereco fixado) ou a recusa.
pub async fn request_checked<F, Fut>(
    spec: &HttpRequestSpec,
    mut check: F,
) -> Result<HttpResult, EgressError>
where
    F: FnMut(Url) -> Fut,
    Fut: std::future::Future<Output = Result<HttpOptions, EgressError>>,
{
    if spec.accept_invalid_certs {
        return Err(EgressError::InvalidCertsDenied);
    }
    let mut url = Url::parse(&spec.url)?;
    let mut salto = spec.clone();
    salto.follow_redirects = false;
    let limite = if spec.follow_redirects {
        spec.max_redirects
    } else {
        0
    };
    for n in 0..=limite {
        let opcoes = check(url.clone()).await?;
        salto.url = url.to_string();
        let resposta = http_request_with(&salto, &opcoes).await?;
        let redirect = matches!(resposta.status, 301 | 302 | 303 | 307 | 308);
        if !spec.follow_redirects || !redirect {
            return Ok(resposta);
        }
        let Some(destino) = resposta.headers.get("location").and_then(|v| v.first()) else {
            return Ok(resposta);
        };
        if n == limite {
            return Err(EgressError::TooManyRedirects(limite));
        }
        let proximo = url.join(destino)?;
        if origin(&proximo) != origin(&url) {
            // Credencial de uma origem nao viaja para outra. Lista do que PASSA, nao do
            // que se tira: a lista de proibidos esquecia chave em cabecalho proprio
            // (X-Subscription-Token do Brave, x-api-key, x-goog-api-key).
            salto.auth = Default::default();
            salto.headers.retain(|k, _| {
                ["accept", "accept-language", "content-type", "user-agent"]
                    .iter()
                    .any(|ok| k.eq_ignore_ascii_case(ok))
            });
        }
        if resposta.status == 303
            || (matches!(resposta.status, 301 | 302) && salto.method.eq_ignore_ascii_case("POST"))
        {
            salto.method = "GET".into();
            salto.body_base64 = None;
            salto.json = None;
            salto.form.clear();
            salto.multipart_fields.clear();
            salto.multipart_files.clear();
        }
        url = proximo;
    }
    Err(EgressError::TooManyRedirects(limite))
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
        assert!(matches!(
            broker.validate_url("https://example.com"),
            Err(EgressError::Disabled)
        ));
    }

    #[test]
    fn exact_origin_allowlist() {
        let mut policy = EgressPolicy {
            enabled: true,
            ..EgressPolicy::default()
        };
        policy
            .allowed_origins
            .insert("https://api.example.com".into());
        let broker = EgressBroker::new(policy);
        assert!(
            broker
                .validate_url("https://api.example.com/v1/test")
                .is_ok()
        );
        assert!(broker.validate_url("https://example.com").is_err());
    }

    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Servidor HTTP minimo: responde `resposta` a cada conexao e conta quantas recebeu.
    fn servidor(resposta: String) -> (String, Arc<AtomicUsize>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", l.local_addr().unwrap());
        let conexoes = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&conexoes);
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                c.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(resposta.as_bytes());
            }
        });
        (addr, conexoes)
    }

    fn broker(origens: &[&str]) -> EgressBroker {
        let mut policy = EgressPolicy {
            enabled: true,
            allow_http: true,
            ..EgressPolicy::default()
        };
        for o in origens {
            policy.allowed_origins.insert((*o).into());
        }
        EgressBroker::new(policy)
    }

    #[tokio::test]
    async fn redirect_para_origem_fora_da_lista_e_recusado_sem_tocar_o_alvo() {
        let (interno, toques) =
            servidor("HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nsegredo".into());
        let (permitido, _) = servidor(format!(
            "HTTP/1.1 302 Found\r\nLocation: {interno}/latest/meta-data\r\nContent-Length: 0\r\n\r\n"
        ));
        let b = broker(&[&permitido]);
        let r = b
            .request(&HttpRequestSpec::get(format!("{permitido}/x")))
            .await;
        assert!(matches!(r, Err(EgressError::OriginDenied(_))), "{r:?}");
        assert_eq!(
            toques.load(Ordering::SeqCst),
            0,
            "o alvo interno recebeu conexao"
        );
    }

    #[tokio::test]
    async fn redirect_dentro_da_lista_e_seguido() {
        let (destino, _) = servidor("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".into());
        let (origem, _) = servidor(format!(
            "HTTP/1.1 302 Found\r\nLocation: {destino}/fim\r\nContent-Length: 0\r\n\r\n"
        ));
        let b = broker(&[&origem, &destino]);
        let r = b
            .request(&HttpRequestSpec::get(format!("{origem}/inicio")))
            .await
            .unwrap();
        assert_eq!(r.status, 200);
        assert!(r.final_url.ends_with("/fim"), "{}", r.final_url);
    }

    #[tokio::test]
    async fn certificado_invalido_nao_se_aceita_por_pedido() {
        let b = broker(&["https://api.example.com"]);
        let mut spec = HttpRequestSpec::get("https://api.example.com/x");
        spec.accept_invalid_certs = true;
        assert!(matches!(
            b.request(&spec).await,
            Err(EgressError::InvalidCertsDenied)
        ));
    }

    #[tokio::test]
    async fn chave_em_cabecalho_proprio_nao_atravessa_redirect_para_outra_origem() {
        // destino guarda o pedido que recebeu
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let destino = format!("http://{}", l.local_addr().unwrap());
        let visto = Arc::new(std::sync::Mutex::new(String::new()));
        let v2 = Arc::clone(&visto);
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                *v2.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });
        let (origem, _) = servidor(format!(
            "HTTP/1.1 302 Found\r\nLocation: {destino}/x\r\nContent-Length: 0\r\n\r\n"
        ));
        let b = broker(&[&origem, &destino]);
        let mut spec = HttpRequestSpec::get(format!("{origem}/inicio"));
        for (k, v) in [
            ("X-Subscription-Token", "chave-brave"),
            ("x-api-key", "chave-a"),
            ("x-goog-api-key", "chave-g"),
            ("Accept", "text/html"),
        ] {
            spec.headers.insert(k.into(), v.into());
        }
        let r = b.request(&spec).await.unwrap();
        assert_eq!(r.status, 200);
        let pedido = visto.lock().unwrap().clone();
        assert!(
            !pedido.contains("chave-"),
            "credencial atravessou a origem: {pedido}"
        );
        assert!(pedido.contains("accept: text/html"), "{pedido}");
    }
}
