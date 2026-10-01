#![forbid(unsafe_code)]
//! Busca na web e leitura de pagina para o agente pesquisar.
//!
//! Todo HTTP desta crate passa pelo `EgressBroker` — nenhuma linha fala com o `reqwest`
//! direto. E o mesmo motor de proposito: o broker e quem confere a origem contra a lista,
//! segue os redirects salto a salto reconferindo cada um e recusa certificado invalido.
//! Um atalho aqui que chamasse o cliente HTTP sozinho seria a porta dos fundos de SSRF
//! que o broker existe para fechar, e nenhum teste do broker a acusaria.

mod html;

pub use html::{PageLink, Readable, decode_entities, inline_text, readable};

use phxclaw_agent_core::BoxFut;
use phxclaw_egress_broker::{EgressBroker, EgressError};
use phxclaw_http_client::{HttpRequestSpec, HttpResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Arc;
use thiserror::Error;
use url::Url;

/// Teto do corpo lido por `fetch_readable`. Pagina de texto util cabe folgada em 2 MB;
/// acima disso quase sempre e binario rotulado errado ou pagina gerada sem fim.
pub const DEFAULT_MAX_BYTES: usize = 2 * 1024 * 1024;

/// Agente de usuario honesto quanto a quem pede, mas no formato que os buscadores
/// aceitam: o `PhxClaw/0.8` cru do cliente HTTP e recusado por alguns como robo mudo.
const USER_AGENT: &str = "Mozilla/5.0 (compatible; PhxClaw-web-search/0.70)";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// URL depois dos redirects que o broker aceitou.
    pub final_url: String,
    pub title: String,
    pub text: String,
    pub links: Vec<PageLink>,
    pub content_type: String,
    /// O corpo passou do teto e foi cortado nele; o texto e parcial.
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error(transparent)]
    Egress(#[from] EgressError),
    #[error("resposta HTTP {status} de {url}: {trecho}")]
    Status {
        status: u16,
        url: String,
        trecho: String,
    },
    #[error("tipo de conteudo nao aceito para leitura: {0}")]
    ContentType(String),
    #[error("resposta ilegivel de {backend}: {motivo}")]
    Parse {
        backend: &'static str,
        motivo: String,
    },
    #[error("o buscador recusou o pedido como automatizado (desafio anti-robo) em {0}")]
    Blocked(String),
    #[error("todas as origens do buscador falharam: {}", .0.join(" | "))]
    AllFailed(Vec<String>),
    #[error("configuracao invalida: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, WebSearchError>;

/// Um buscador. Devolve `BoxFut` e nao `async fn` para ser usavel como `dyn`, como o
/// resto do contrato do agente em `phxclaw-agent-core`.
pub trait SearchBackend: Send + Sync {
    fn name(&self) -> &'static str;
    /// Origens que a politica de egresso precisa permitir para este buscador funcionar.
    /// Existe para quem monta a politica nao ter de adivinhar pelo codigo.
    fn origins(&self) -> Vec<String>;
    fn search<'a>(&'a self, query: &'a str, max: usize) -> BoxFut<'a, Result<Vec<SearchHit>>>;
}

/// Escolhe o buscador pelas variaveis de ambiente: `PHXCLAW_SEARXNG_URL` (instancia
/// propria, sem chave e sem cota) vence; depois Brave se houver `BRAVE_API_KEY`; e o
/// DuckDuckGo HTML fica por ultimo porque nao tem contrato — o layout muda sem aviso.
pub fn from_env(broker: Arc<EgressBroker>) -> Result<Box<dyn SearchBackend>> {
    from_vars(
        |k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()),
        broker,
    )
}

/// O mesmo que `from_env`, com a leitura das variaveis injetada — testavel sem mexer no
/// ambiente do processo (que no Rust 2024 exige `unsafe`).
pub fn from_vars(
    var: impl Fn(&str) -> Option<String>,
    broker: Arc<EgressBroker>,
) -> Result<Box<dyn SearchBackend>> {
    if let Some(base) = var("PHXCLAW_SEARXNG_URL") {
        return Ok(Box::new(SearxngBackend::new(broker, &base)?));
    }
    if let Some(chave) = var("BRAVE_API_KEY") {
        return Ok(Box::new(BraveBackend::new(broker, chave)));
    }
    Ok(Box::new(DuckDuckGoHtmlBackend::new(broker)))
}

fn origin_of(u: &Url) -> String {
    match u.port() {
        Some(p) => format!("{}://{}:{}", u.scheme(), u.host_str().unwrap_or(""), p),
        None => format!("{}://{}", u.scheme(), u.host_str().unwrap_or("")),
    }
}

fn spec(url: &str) -> HttpRequestSpec {
    let mut s = HttpRequestSpec::get(url);
    s.user_agent = Some(USER_AGENT.into());
    s.timeout_ms = 20_000;
    s
}

/// Unico ponto de saida da crate: broker, e status fora de 2xx vira erro com um trecho do
/// corpo — o motivo que o servidor escreveu vale mais que o numero sozinho.
async fn send(broker: &EgressBroker, s: &HttpRequestSpec) -> Result<HttpResult> {
    let r = broker.request(s).await?;
    if !(200..300).contains(&r.status) {
        let corpo = r.bytes().unwrap_or_default();
        let trecho: String = String::from_utf8_lossy(&corpo).chars().take(200).collect();
        return Err(WebSearchError::Status {
            status: r.status,
            url: r.final_url,
            trecho: html::collapse_ws(&trecho),
        });
    }
    Ok(r)
}

fn body_text(r: &HttpResult, backend: &'static str) -> Result<String> {
    let b = r.bytes().map_err(|e| WebSearchError::Parse {
        backend,
        motivo: e.to_string(),
    })?;
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// So URL que o agente consegue abrir entra na lista, e cada uma uma vez so: resultado
/// repetido gasta uma das `max` vagas sem trazer nada novo.
fn push_hit(hits: &mut Vec<SearchHit>, vistos: &mut BTreeSet<String>, h: SearchHit) {
    let Ok(u) = Url::parse(&h.url) else { return };
    if !matches!(u.scheme(), "http" | "https") || h.title.is_empty() {
        return;
    }
    if vistos.insert(u.to_string()) {
        hits.push(SearchHit {
            url: u.to_string(),
            ..h
        });
    }
}

// ---------------------------------------------------------------- SearXNG

pub struct SearxngBackend {
    broker: Arc<EgressBroker>,
    endpoint: Url,
}

impl SearxngBackend {
    pub fn new(broker: Arc<EgressBroker>, base: &str) -> Result<Self> {
        let mut base = Url::parse(base.trim())
            .map_err(|e| WebSearchError::Config(format!("PHXCLAW_SEARXNG_URL: {e}")))?;
        // Instancia pode morar num subcaminho (`https://host/searx/`); `join("search")`
        // sem a barra final trocaria o ultimo segmento em vez de acrescentar.
        if !base.path().ends_with('/') {
            let p = format!("{}/", base.path());
            base.set_path(&p);
        }
        let endpoint = base
            .join("search")
            .map_err(|e| WebSearchError::Config(e.to_string()))?;
        Ok(Self { broker, endpoint })
    }
}

pub fn parse_searxng(json: &str, max: usize) -> Result<Vec<SearchHit>> {
    let v: Value = serde_json::from_str(json).map_err(|e| WebSearchError::Parse {
        backend: "searxng",
        motivo: e.to_string(),
    })?;
    let Some(itens) = v.get("results").and_then(Value::as_array) else {
        return Err(WebSearchError::Parse {
            backend: "searxng",
            motivo: "sem o campo `results` (a instancia tem `format=json` habilitado?)".into(),
        });
    };
    let mut hits = Vec::new();
    let mut vistos = BTreeSet::new();
    for it in itens {
        if hits.len() >= max {
            break;
        }
        let campo = |k: &str| it.get(k).and_then(Value::as_str).unwrap_or("");
        push_hit(
            &mut hits,
            &mut vistos,
            SearchHit {
                title: inline_text(campo("title")),
                url: campo("url").to_string(),
                snippet: inline_text(campo("content")),
            },
        );
    }
    Ok(hits)
}

impl SearchBackend for SearxngBackend {
    fn name(&self) -> &'static str {
        "searxng"
    }
    fn origins(&self) -> Vec<String> {
        vec![origin_of(&self.endpoint)]
    }
    fn search<'a>(&'a self, query: &'a str, max: usize) -> BoxFut<'a, Result<Vec<SearchHit>>> {
        Box::pin(async move {
            let mut s = spec(self.endpoint.as_str());
            s.query = vec![("q".into(), query.into()), ("format".into(), "json".into())];
            s.headers.insert("Accept".into(), "application/json".into());
            let r = send(&self.broker, &s).await?;
            parse_searxng(&body_text(&r, "searxng")?, max)
        })
    }
}

// ---------------------------------------------------------------- Brave

pub const BRAVE_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

pub struct BraveBackend {
    broker: Arc<EgressBroker>,
    endpoint: String,
    chave: String,
}

impl BraveBackend {
    pub fn new(broker: Arc<EgressBroker>, chave: String) -> Self {
        Self::with_endpoint(broker, chave, BRAVE_ENDPOINT)
    }
    pub fn with_endpoint(broker: Arc<EgressBroker>, chave: String, endpoint: &str) -> Self {
        Self {
            broker,
            endpoint: endpoint.into(),
            chave,
        }
    }
}

impl std::fmt::Debug for BraveBackend {
    // A chave nunca aparece em `{:?}`: log de depuracao e o lugar onde segredo vaza.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BraveBackend")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

pub fn parse_brave(json: &str, max: usize) -> Result<Vec<SearchHit>> {
    let v: Value = serde_json::from_str(json).map_err(|e| WebSearchError::Parse {
        backend: "brave",
        motivo: e.to_string(),
    })?;
    // Sem `web` e resposta valida de busca sem resultado de pagina (so noticias, por
    // exemplo): lista vazia, nao erro.
    let itens = v
        .pointer("/web/results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut hits = Vec::new();
    let mut vistos = BTreeSet::new();
    for it in &itens {
        if hits.len() >= max {
            break;
        }
        let campo = |k: &str| it.get(k).and_then(Value::as_str).unwrap_or("");
        push_hit(
            &mut hits,
            &mut vistos,
            SearchHit {
                title: inline_text(campo("title")),
                url: campo("url").to_string(),
                snippet: inline_text(campo("description")),
            },
        );
    }
    Ok(hits)
}

impl SearchBackend for BraveBackend {
    fn name(&self) -> &'static str {
        "brave"
    }
    fn origins(&self) -> Vec<String> {
        Url::parse(&self.endpoint)
            .map(|u| vec![origin_of(&u)])
            .unwrap_or_default()
    }
    fn search<'a>(&'a self, query: &'a str, max: usize) -> BoxFut<'a, Result<Vec<SearchHit>>> {
        Box::pin(async move {
            let mut s = spec(&self.endpoint);
            // A API aceita no maximo 20 por pagina.
            let n = max.clamp(1, 20);
            s.query = vec![("q".into(), query.into()), ("count".into(), n.to_string())];
            s.headers.insert("Accept".into(), "application/json".into());
            s.headers
                .insert("X-Subscription-Token".into(), self.chave.clone());
            // O broker tira `Authorization` e `Cookie` num redirect entre origens, mas nao
            // conhece este cabecalho: seguir redirect levaria a chave junto para onde a
            // API mandasse. A API nao redireciona; se um dia redirecionar, vira erro.
            s.follow_redirects = false;
            let r = send(&self.broker, &s).await?;
            parse_brave(&body_text(&r, "brave")?, max)
        })
    }
}

// ---------------------------------------------------------------- DuckDuckGo

pub const DDG_HTML: &str = "https://html.duckduckgo.com/html/";
pub const DDG_LITE: &str = "https://lite.duckduckgo.com/lite/";

/// DuckDuckGo pela pagina HTML sem JavaScript. Tenta as origens em ordem: a `html` tem
/// trecho melhor; a `lite` e o plano B quando a primeira recusa ou muda de layout.
pub struct DuckDuckGoHtmlBackend {
    broker: Arc<EgressBroker>,
    endpoints: Vec<String>,
}

impl DuckDuckGoHtmlBackend {
    pub fn new(broker: Arc<EgressBroker>) -> Self {
        Self::with_endpoints(broker, vec![DDG_HTML.into(), DDG_LITE.into()])
    }
    pub fn with_endpoints(broker: Arc<EgressBroker>, endpoints: Vec<String>) -> Self {
        Self { broker, endpoints }
    }

    async fn search_one(&self, endpoint: &str, query: &str, max: usize) -> Result<Vec<SearchHit>> {
        let mut s = spec(endpoint);
        // POST com formulario e o que a propria pagina faz; GET com `q` na URL e o
        // caminho que o DuckDuckGo mais cedo passa a tratar como robo.
        s.method = "POST".into();
        s.form = vec![("q".into(), query.into())];
        s.headers.insert(
            "Accept".into(),
            "text/html,application/xhtml+xml;q=0.9".into(),
        );
        let r = send(&self.broker, &s).await?;
        let corpo = body_text(&r, "duckduckgo")?;
        let hits = parse_duckduckgo(&corpo, max);
        if hits.is_empty() && is_ddg_challenge(&corpo) {
            return Err(WebSearchError::Blocked(r.final_url));
        }
        Ok(hits)
    }
}

/// Pagina de desafio do DuckDuckGo: responde 200/202 com um formulario de captcha no lugar
/// dos resultados. Sem reconhecer isto, a recusa chegaria ao agente como "nenhum
/// resultado" — e ele concluiria que o assunto nao existe.
fn is_ddg_challenge(corpo: &str) -> bool {
    ["anomaly-modal", "challenge-form", "bots use DuckDuckGo"]
        .iter()
        .any(|m| corpo.contains(m))
}

/// O link de resultado do DuckDuckGo passa pelo redirecionador `/l/?uddg=<url codificada>`.
/// Devolver o redirecionador faria o agente abrir o DuckDuckGo em vez da pagina, e o
/// broker recusaria a origem do destino que ninguem viu. Anuncio (`/y.js`) nao tem
/// `uddg` e fica no dominio do DuckDuckGo: cai no filtro de host.
pub fn ddg_target(href: &str) -> Option<String> {
    let base = Url::parse("https://duckduckgo.com/").ok()?;
    let u = base.join(href.trim()).ok()?;
    let alvo = match u.query_pairs().find(|(k, _)| k == "uddg") {
        Some((_, v)) => Url::parse(&v).ok()?,
        None => u,
    };
    let host = alvo.host_str()?;
    if !matches!(alvo.scheme(), "http" | "https")
        || host == "duckduckgo.com"
        || host.ends_with(".duckduckgo.com")
    {
        return None;
    }
    Some(alvo.to_string())
}

/// Le as duas variantes do DuckDuckGo sem JavaScript: `html` (`a.result__a` +
/// `.result__snippet`) e `lite` (`a.result-link` + `td.result-snippet`).
pub fn parse_duckduckgo(corpo: &str, max: usize) -> Vec<SearchHit> {
    use html::Token;
    #[derive(PartialEq)]
    enum Modo {
        Fora,
        Titulo,
        Trecho { tag: String, prof: usize },
    }
    let mut hits = Vec::new();
    let mut vistos = BTreeSet::new();
    let mut atual: Option<SearchHit> = None;
    let mut modo = Modo::Fora;
    let fecha = |atual: &mut Option<SearchHit>, hits: &mut Vec<_>, vistos: &mut _| {
        if let Some(mut h) = atual.take() {
            h.title = html::collapse_ws(&h.title);
            h.snippet = html::collapse_ws(&h.snippet);
            push_hit(hits, vistos, h);
        }
    };
    for t in html::tokenize(corpo) {
        match &t {
            Token::Start { name, .. } if matches!(modo, Modo::Fora) => {
                if name == "a" && (t.has_class("result__a") || t.has_class("result-link")) {
                    fecha(&mut atual, &mut hits, &mut vistos);
                    if hits.len() >= max {
                        return hits;
                    }
                    atual = t.attr("href").and_then(ddg_target).map(|url| SearchHit {
                        title: String::new(),
                        url,
                        snippet: String::new(),
                    });
                    modo = Modo::Titulo;
                } else if t.has_class("result__snippet") || t.has_class("result-snippet") {
                    modo = Modo::Trecho {
                        tag: name.clone(),
                        prof: 1,
                    };
                }
            }
            Token::Start {
                name,
                self_closing: false,
                ..
            } => {
                if let Modo::Trecho { tag, prof } = &mut modo
                    && tag == name
                {
                    *prof += 1;
                }
            }
            Token::End { name } => match &mut modo {
                Modo::Titulo if name == "a" => modo = Modo::Fora,
                Modo::Trecho { tag, prof } if tag == name => {
                    *prof -= 1;
                    if *prof == 0 {
                        modo = Modo::Fora;
                    }
                }
                _ => {}
            },
            Token::Text(s) => {
                if let Some(h) = atual.as_mut() {
                    match modo {
                        Modo::Titulo => h.title.push_str(&decode_entities(s)),
                        Modo::Trecho { .. } => {
                            h.snippet.push_str(&decode_entities(s));
                        }
                        Modo::Fora => {}
                    }
                }
            }
            _ => {}
        }
    }
    fecha(&mut atual, &mut hits, &mut vistos);
    hits.truncate(max);
    hits
}

impl SearchBackend for DuckDuckGoHtmlBackend {
    fn name(&self) -> &'static str {
        "duckduckgo"
    }
    fn origins(&self) -> Vec<String> {
        self.endpoints
            .iter()
            .filter_map(|e| Url::parse(e).ok())
            .map(|u| origin_of(&u))
            .collect()
    }
    fn search<'a>(&'a self, query: &'a str, max: usize) -> BoxFut<'a, Result<Vec<SearchHit>>> {
        Box::pin(async move {
            let mut falhas = Vec::new();
            for e in &self.endpoints {
                match self.search_one(e, query, max).await {
                    Ok(h) if !h.is_empty() => return Ok(h),
                    Ok(_) => falhas.push(format!("{e}: 0 resultados")),
                    Err(err) => falhas.push(format!("{e}: {err}")),
                }
            }
            // Zero resultados em todas as origens e resposta, nao falha: o assunto pode
            // mesmo nao ter pagina. So vira erro quando alguma origem de fato falhou.
            if falhas.iter().all(|f| f.ends_with(": 0 resultados")) {
                return Ok(Vec::new());
            }
            Err(WebSearchError::AllFailed(falhas))
        })
    }
}

// ---------------------------------------------------------------- leitura de pagina

pub async fn fetch_readable(broker: &EgressBroker, url: &str) -> Result<Page> {
    fetch_readable_with(broker, url, DEFAULT_MAX_BYTES).await
}

/// Baixa pelo broker e devolve o texto legivel. Aceita so `text/html` e `text/plain`:
/// PDF, imagem e binario sem rotulo virariam lixo no contexto do modelo, e cheirar o
/// tipo pelo conteudo e exatamente o que abre porta para confusao de tipo.
///
/// Limite de bytes: o cliente HTTP comum entrega o corpo inteiro de uma vez (nao ha
/// leitura em fluxo no broker), entao o teto aqui protege o parse e o contexto do
/// modelo, nao a memoria do download. Cortar no teto em vez de recusar e escolha: o
/// comeco de uma pagina grande ainda responde a pergunta, e `truncated` avisa.
pub async fn fetch_readable_with(
    broker: &EgressBroker,
    url: &str,
    max_bytes: usize,
) -> Result<Page> {
    let mut s = spec(url);
    s.headers.insert(
        "Accept".into(),
        "text/html,application/xhtml+xml;q=0.9,text/plain;q=0.8".into(),
    );
    let r = send(broker, &s).await?;
    let ct_bruto = r.content_type.clone().unwrap_or_default();
    let ct = ct_bruto
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if ct != "text/html" && ct != "text/plain" {
        return Err(WebSearchError::ContentType(if ct.is_empty() {
            "(ausente)".into()
        } else {
            ct_bruto
        }));
    }
    let mut corpo = r.bytes().map_err(|e| WebSearchError::Parse {
        backend: "fetch",
        motivo: e.to_string(),
    })?;
    let truncated = corpo.len() > max_bytes;
    corpo.truncate(max_bytes);
    let texto = String::from_utf8_lossy(&corpo);
    let base = Url::parse(&r.final_url).ok();
    let (title, text, links) = if ct == "text/html" {
        let rd = html::readable(&texto, base.as_ref());
        (rd.title, rd.text, rd.links)
    } else {
        (String::new(), html::plain(&texto), Vec::new())
    };
    Ok(Page {
        final_url: r.final_url,
        title,
        text,
        links,
        content_type: ct,
        truncated,
    })
}

#[cfg(test)]
mod tests;
