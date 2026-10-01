//! Credencial dos servidores MCP remotos: Bearer fixo (Linear) e OAuth 2.0 com PKCE pelo
//! navegador local (Gmail, Drive e Calendar do Google Workspace).
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Todo segredo mora no SecretBroker** da pasta `mcp/` do agente, pela mesma
//!   `Credencial` dos canais e das forjas (concessao de 30 s, limpeza do erro). O token de
//!   acesso do OAuth tambem: o que fica em memoria e so QUANDO ele vence, nunca ele.
//! - **PKCE (RFC 7636) sempre, S256 sempre.** O `plain` nao e oferecido: ele so existe para
//!   cliente que nao calcula SHA-256, e aqui o SHA-256 ja esta na arvore.
//! - **Redirecionamento por loopback (RFC 8252 §7.3)**: `http://127.0.0.1:<porta>/callback`
//!   com porta efemera, escutando so em 127.0.0.1. O `state` confere a volta: pedido com
//!   `state` errado e recusado e a espera continua (qualquer processo da maquina alcanca a
//!   porta), em vez de aceitar o primeiro que chegar.
//! - **O navegador nao e aberto por aqui**: a URL e entregue a quem chamou (a CLI a imprime).
//!   Abrir exigiria criar processo fora do sandbox, e a pétrea do bwrap pede excecao
//!   declarada para isso; copiar a URL custa um gesto ao operador e nenhuma excecao.
//! - **Renovacao (refresh) na hora**: antes de cada pedido, se o acesso venceu ou vence em
//!   menos de um minuto; e depois de um 401, uma vez. Refresh token rotacionado pelo
//!   servidor substitui o antigo no broker (o Google nao rotaciona; outros sim).

use crate::canais::http::Credencial;
use crate::canais::{guardar_segredo, segredo_guardado};
use base64::Engine;
use phxclaw_http_client::{HttpRequestSpec, http_request};
use phxclaw_mcp_lsp_runtime::AuthorizationSource;
use phxclaw_secret_broker::{SecretBroker, SecretValue, scrub_text};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const NAMESPACE: &str = "mcp";
const CONSUMIDOR: &str = "phxclaw.agent.mcp";
/// Antes do vencimento declarado: o relogio do servidor e o nosso nao batem ao segundo, e
/// um pedido longo comecado com token quase vencido chegaria vencido.
const FOLGA_DE_VENCIMENTO: Duration = Duration::from_secs(60);
/// Sem `expires_in` na resposta, o token vale por isto do nosso lado; o 401 corrige o resto.
const VALIDADE_PADRAO: Duration = Duration::from_secs(3000);

/// A pasta do broker dos servidores MCP, ao lado das tarefas e da `forja/`.
pub fn pasta_do_mcp(raiz_do_agente: &Path) -> PathBuf {
    raiz_do_agente.join("mcp")
}

/// Como o operador declara o OAuth de um servidor. Os endpoints do Google vem do `preset`
/// (ver `mcp::ServidorDeclarado`); o `cliente_id` e sempre do operador.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigOauth {
    #[serde(default)]
    pub autorizacao: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub cliente_id: String,
    #[serde(default)]
    pub escopos: Vec<String>,
    /// O cliente tem segredo (o «Web application» do Google exige). Ele e pedido no
    /// `phxclaw mcp login` e vai para o broker, nunca para este arquivo.
    #[serde(default)]
    pub segredo_cliente: bool,
    /// Parametros a mais na URL de autorizacao (`access_type=offline` no Google, sem o qual
    /// nao volta refresh token).
    #[serde(default)]
    pub parametros: BTreeMap<String, String>,
}

impl ConfigOauth {
    /// Endpoint de token so por HTTPS, ou HTTP em loopback (o servidor falso dos testes, um
    /// proxy local): o refresh token e o segredo do cliente vao no corpo desse pedido.
    pub fn validar(&self) -> Result<(), String> {
        if self.cliente_id.trim().is_empty() {
            return Err("oauth sem 'cliente_id'".into());
        }
        for (nome, u) in [("autorizacao", &self.autorizacao), ("token", &self.token)] {
            let url = reqwest::Url::parse(u).map_err(|e| format!("oauth '{nome}': {e}"))?;
            let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
            if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
                return Err(format!("oauth '{nome}': so HTTPS (HTTP so em loopback)"));
            }
        }
        Ok(())
    }
}

// ------------------------------------------------------------------ PKCE

fn base64url(b: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Texto aleatorio de 32 bytes em base64url (43 caracteres, todos «unreserved»): serve de
/// `code_verifier` (RFC 7636 §4.1 pede 43 a 128) e de `state`.
pub fn aleatorio() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| format!("sem fonte aleatoria: {e}"))?;
    Ok(base64url(&b))
}

/// `code_challenge` S256 (RFC 7636 §4.2): base64url sem preenchimento do SHA-256 do
/// verificador em ASCII.
pub fn desafio(verificador: &str) -> String {
    base64url(&Sha256::digest(verificador.as_bytes()))
}

/// A URL que o operador abre no navegador.
pub fn url_de_autorizacao(
    cfg: &ConfigOauth,
    redirecionamento: &str,
    state: &str,
    desafio: &str,
) -> Result<String, String> {
    let mut u = reqwest::Url::parse(&cfg.autorizacao).map_err(|e| e.to_string())?;
    {
        let mut q = u.query_pairs_mut();
        q.append_pair("response_type", "code")
            .append_pair("client_id", &cfg.cliente_id)
            .append_pair("redirect_uri", redirecionamento)
            .append_pair("state", state)
            .append_pair("code_challenge", desafio)
            .append_pair("code_challenge_method", "S256");
        if !cfg.escopos.is_empty() {
            q.append_pair("scope", &cfg.escopos.join(" "));
        }
        for (k, v) in &cfg.parametros {
            q.append_pair(k, v);
        }
    }
    Ok(u.to_string())
}

/// Espera a volta do navegador no loopback e devolve o `code`. So aceita `GET /callback`
/// com o `state` combinado; o resto recebe 404/400 e a espera continua ate o prazo.
pub async fn esperar_codigo(
    escuta: tokio::net::TcpListener,
    state: &str,
    prazo: Duration,
) -> Result<String, String> {
    let fim = tokio::time::Instant::now() + prazo;
    loop {
        let (mut s, _) = tokio::time::timeout_at(fim, escuta.accept())
            .await
            .map_err(|_| format!("o navegador nao voltou em {}s", prazo.as_secs()))?
            .map_err(|e| format!("loopback: {e}"))?;
        let mut cabeca = Vec::new();
        let mut bloco = [0u8; 1024];
        // So a linha do pedido interessa; o teto impede um cliente da maquina de mandar o
        // agente alocar o que quiser.
        while !cabeca.windows(4).any(|j| j == b"\r\n\r\n") && cabeca.len() < 8192 {
            let n = match tokio::time::timeout(Duration::from_secs(5), s.read(&mut bloco)).await {
                Ok(Ok(n)) if n > 0 => n,
                _ => break,
            };
            cabeca.extend_from_slice(&bloco[..n]);
        }
        let linha = String::from_utf8_lossy(&cabeca)
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let caminho = linha
            .strip_prefix("GET ")
            .and_then(|r| r.split(' ').next())
            .unwrap_or("");
        let url = reqwest::Url::parse(&format!("http://127.0.0.1{caminho}")).ok();
        let (status, texto, desfecho) = match url.filter(|u| u.path() == "/callback") {
            None => ("404 Not Found", "nada aqui", None),
            Some(u) => {
                let q: BTreeMap<String, String> = u.query_pairs().into_owned().collect();
                if q.get("state").map(String::as_str) != Some(state) {
                    (
                        "400 Bad Request",
                        "state nao confere: pedido recusado",
                        None,
                    )
                } else if let Some(erro) = q.get("error") {
                    (
                        "400 Bad Request",
                        "autorizacao negada; volte ao terminal",
                        Some(Err(format!("o servidor negou a autorizacao: {erro}"))),
                    )
                } else if let Some(c) = q.get("code").filter(|c| !c.is_empty()) {
                    (
                        "200 OK",
                        "PhxClaw autorizado. Pode fechar esta aba e voltar ao terminal.",
                        Some(Ok(c.clone())),
                    )
                } else {
                    ("400 Bad Request", "faltou o code", None)
                }
            }
        };
        let corpo = format!("<!doctype html><meta charset=utf-8><p>{texto}</p>");
        let _ = s
            .write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
                    corpo.len()
                )
                .as_bytes(),
            )
            .await;
        let _ = s.shutdown().await;
        if let Some(d) = desfecho {
            return d;
        }
    }
}

/// O que o endpoint de token devolve, sem o valor: os tokens vao direto para o broker.
struct Resposta {
    acesso: SecretValue,
    renovacao: Option<SecretValue>,
    validade: Duration,
}

/// Um POST ao endpoint de token (RFC 6749 §4.1.3 e §6). Os segredos do corpo saem de todo
/// erro: servidor de token que ecoa o pedido no erro devolveria o refresh token ao log.
async fn pedir_token(
    cfg: &ConfigOauth,
    mut form: Vec<(String, String)>,
    cliente: Option<&str>,
) -> Result<Resposta, String> {
    form.push(("client_id".into(), cfg.cliente_id.clone()));
    if let Some(s) = cliente {
        form.push(("client_secret".into(), s.to_string()));
    }
    let segredos: Vec<SecretValue> = form
        .iter()
        .filter(|(k, _)| {
            matches!(
                k.as_str(),
                "code" | "code_verifier" | "refresh_token" | "client_secret"
            )
        })
        .map(|(_, v)| SecretValue::new(v.clone()))
        .collect();
    let limpo = |m: String| scrub_text(&m, &segredos);
    let mut spec = HttpRequestSpec::get(cfg.token.clone());
    spec.method = "POST".into();
    spec.form = form;
    spec.follow_redirects = false;
    spec.headers
        .insert("Accept".into(), "application/json".into());
    spec.user_agent = Some(concat!("PhxClaw/", env!("CARGO_PKG_VERSION")).into());
    let r = http_request(&spec)
        .await
        .map_err(|e| limpo(format!("endpoint de token: {e}")));
    drop(spec);
    let r = r?;
    let corpo = r.text().unwrap_or_default();
    let v: Value = serde_json::from_str(&corpo).unwrap_or(Value::Null);
    if r.status != 200 {
        // O `error` do RFC 6749 §5.2 (invalid_grant...) diz o que houve sem ecoar nada.
        let erro = v
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| corpo.chars().take(200).collect());
        return Err(limpo(format!(
            "endpoint de token respondeu {}: {erro}",
            r.status
        )));
    }
    let acesso = v
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or("endpoint de token sem access_token")?;
    Ok(Resposta {
        acesso: SecretValue::new(acesso.to_string()),
        renovacao: v
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(|t| SecretValue::new(t.to_string())),
        validade: v
            .get("expires_in")
            .and_then(Value::as_u64)
            .map(Duration::from_secs)
            .unwrap_or(VALIDADE_PADRAO),
    })
}

// ------------------------------------------------------------------ guarda no broker

/// Os segredos de um servidor: um por tipo, cada um com o escopo so dele.
#[derive(Clone, Copy)]
enum Tipo {
    Bearer,
    Acesso,
    Renovacao,
    Cliente,
}

impl Tipo {
    fn uso(self) -> &'static str {
        match self {
            Tipo::Bearer => "bearer",
            Tipo::Acesso => "acesso",
            Tipo::Renovacao => "renovacao",
            Tipo::Cliente => "cliente",
        }
    }
}

fn nome_do_segredo(servidor: &str, t: Tipo) -> String {
    format!("mcp-{servidor}-{}", t.uso())
}

fn prefixo(servidor: &str) -> String {
    format!("mcp:{servidor}")
}

fn guardar(
    broker: &SecretBroker,
    servidor: &str,
    t: Tipo,
    valor: SecretValue,
) -> Result<uuid::Uuid, String> {
    let escopo = format!("{}:{}", prefixo(servidor), t.uso());
    guardar_segredo(
        broker,
        &nome_do_segredo(servidor, t),
        NAMESPACE,
        &[&escopo],
        valor,
    )
}

fn credencial(
    broker: &Arc<SecretBroker>,
    servidor: &str,
    t: Tipo,
) -> Result<Option<Credencial>, String> {
    Ok(
        segredo_guardado(broker, &nome_do_segredo(servidor, t), NAMESPACE)?
            .map(|d| Credencial::de_escopo(broker.clone(), d.uuid, CONSUMIDOR, prefixo(servidor))),
    )
}

/// Broker da pasta `mcp/`; so existe depois de um `phxclaw mcp token|login`.
pub fn broker_da_pasta(
    raiz_do_agente: &Path,
    criar: bool,
) -> Result<Option<Arc<SecretBroker>>, String> {
    let pasta = pasta_do_mcp(raiz_do_agente);
    if !criar && !pasta.join("segredos/master.key").exists() {
        return Ok(None);
    }
    crate::canais::broker_em(&pasta).map(Some)
}

/// `phxclaw mcp token NOME`: guarda o Bearer fixo (a chave de API do Linear).
pub fn guardar_bearer(
    raiz_do_agente: &Path,
    servidor: &str,
    token: SecretValue,
) -> Result<uuid::Uuid, String> {
    let b = broker_da_pasta(raiz_do_agente, true)?.ok_or("sem broker")?;
    guardar(&b, servidor, Tipo::Bearer, token)
}

/// O fluxo inteiro do `phxclaw mcp login NOME`: PKCE, loopback, troca do codigo com o
/// verificador, e o refresh token (com o acesso) no broker. `abrir` recebe a URL que o
/// operador abre no navegador; `segredo_cliente` so quando a configuracao o exige.
pub async fn login(
    raiz_do_agente: &Path,
    servidor: &str,
    cfg: &ConfigOauth,
    segredo_cliente: Option<SecretValue>,
    prazo: Duration,
    abrir: impl FnOnce(&str),
) -> Result<(), String> {
    cfg.validar()?;
    if cfg.segredo_cliente && segredo_cliente.is_none() {
        return Err("este cliente OAuth exige o segredo do cliente".into());
    }
    let broker = broker_da_pasta(raiz_do_agente, true)?.ok_or("sem broker")?;
    let escuta = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("loopback: {e}"))?;
    let porta = escuta.local_addr().map_err(|e| e.to_string())?.port();
    let redirecionamento = format!("http://127.0.0.1:{porta}/callback");
    let verificador = aleatorio()?;
    let state = aleatorio()?;
    abrir(&url_de_autorizacao(
        cfg,
        &redirecionamento,
        &state,
        &desafio(&verificador),
    )?);
    let codigo = esperar_codigo(escuta, &state, prazo).await?;
    let r = pedir_token(
        cfg,
        vec![
            ("grant_type".into(), "authorization_code".into()),
            ("code".into(), codigo),
            ("redirect_uri".into(), redirecionamento),
            ("code_verifier".into(), verificador),
        ],
        segredo_cliente.as_ref().map(SecretValue::expose),
    )
    .await?;
    let renovacao = r.renovacao.ok_or(
        "o servidor nao devolveu refresh token (no Google: access_type=offline e prompt=consent)",
    )?;
    guardar(&broker, servidor, Tipo::Renovacao, renovacao)?;
    guardar(&broker, servidor, Tipo::Acesso, r.acesso)?;
    if let Some(s) = segredo_cliente {
        guardar(&broker, servidor, Tipo::Cliente, s)?;
    }
    Ok(())
}

/// `phxclaw mcp token|login NOME`: o servidor sai da MESMA configuracao que o agente carrega
/// (`PHXCLAW_MCP_CONFIG`, com o preset aplicado), para o login e o uso nunca divergirem de
/// endpoint. Os valores vem do ambiente do comando (`PHXCLAW_MCP_TOKEN`,
/// `PHXCLAW_MCP_SEGREDO_CLIENTE`) e vao para o broker; guardar e ato do operador.
pub async fn cli(raiz_do_agente: &Path, args: &[String]) -> Result<String, String> {
    let uso = "uso: phxclaw mcp token|login NOME [--pasta DIR]";
    let (Some(acao), Some(nome)) = (args.first(), args.get(1)) else {
        return Err(uso.into());
    };
    let d = crate::mcp::declarado_no_ambiente(nome)?;
    let servidor = phxclaw_mcp_lsp_runtime::normalize_mcp_name(d.nome.trim());
    let env = |v: &str| {
        std::env::var(v)
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .map(SecretValue::new)
    };
    match (acao.as_str(), &d.auth) {
        ("token", Some(crate::mcp::AuthDeclarada::Bearer)) => {
            let t = env("PHXCLAW_MCP_TOKEN").ok_or("falta PHXCLAW_MCP_TOKEN no ambiente")?;
            let id = guardar_bearer(raiz_do_agente, &servidor, t)?;
            Ok(format!(
                "token de {servidor} guardado no broker de {} (segredo {id})",
                pasta_do_mcp(raiz_do_agente).display()
            ))
        }
        ("login", Some(crate::mcp::AuthDeclarada::Oauth(cfg))) => {
            login(
                raiz_do_agente,
                &servidor,
                cfg,
                env("PHXCLAW_MCP_SEGREDO_CLIENTE"),
                Duration::from_secs(300),
                |url| eprintln!("Abra no navegador para autorizar {servidor}:\n\n  {url}\n"),
            )
            .await?;
            Ok(format!(
                "{servidor} autorizado; refresh token no broker de {}",
                pasta_do_mcp(raiz_do_agente).display()
            ))
        }
        ("token", _) | ("login", _) => Err(format!(
            "{servidor}: a configuracao declara outra credencial (token = auth bearer, login = auth oauth)"
        )),
        _ => Err(uso.into()),
    }
}

// ------------------------------------------------------------------ fonte do Authorization

/// O `Authorization` de um servidor MCP, pedido a cada pedido pelo cliente HTTP do runtime.
pub enum AutorizacaoMcp {
    Bearer(Credencial),
    Oauth(Box<Oauth>),
}

pub struct Oauth {
    servidor: String,
    cfg: ConfigOauth,
    broker: Arc<SecretBroker>,
    /// Quando o acesso guardado deixa de valer; `None` = nao se sabe (processo novo), e a
    /// primeira chamada renova. A trava tambem serializa a renovacao: duas tarefas com o
    /// token vencido nao gastam dois refresh (e, com rotacao, um invalidaria o outro).
    vence: tokio::sync::Mutex<Option<Instant>>,
}

impl AutorizacaoMcp {
    /// A credencial declarada para `servidor`, se o operador ja guardou o segredo. `None`
    /// com erro explicado quando falta: servidor que exige credencial nao sobe sem ela.
    pub fn da_pasta(
        raiz_do_agente: &Path,
        servidor: &str,
        oauth: Option<&ConfigOauth>,
    ) -> Result<Self, String> {
        let falta = |cmd: &str| format!("sem credencial: rode `phxclaw mcp {cmd} {servidor}`");
        let cmd = if oauth.is_some() { "login" } else { "token" };
        let broker = broker_da_pasta(raiz_do_agente, false)?.ok_or_else(|| falta(cmd))?;
        match oauth {
            None => credencial(&broker, servidor, Tipo::Bearer)?
                .map(AutorizacaoMcp::Bearer)
                .ok_or_else(|| falta(cmd)),
            Some(cfg) => {
                cfg.validar()?;
                credencial(&broker, servidor, Tipo::Renovacao)?.ok_or_else(|| falta(cmd))?;
                Ok(AutorizacaoMcp::Oauth(Box::new(Oauth {
                    servidor: servidor.to_string(),
                    cfg: cfg.clone(),
                    broker,
                    vence: tokio::sync::Mutex::new(None),
                })))
            }
        }
    }

    /// Depois de um 401: o acesso guardado deixa de valer, e o proximo pedido renova. No
    /// Bearer fixo nao ha o que renovar -- devolve `false`, e o 401 sobe como esta.
    pub async fn invalidar(&self) -> bool {
        match self {
            AutorizacaoMcp::Bearer(_) => false,
            AutorizacaoMcp::Oauth(o) => {
                *o.vence.lock().await = None;
                true
            }
        }
    }

    /// Tira de `texto` todo segredo deste servidor. O servidor MCP e de terceiro: um que
    /// ecoe o cabecalho no erro ou no resultado nao pode devolver o token ao modelo.
    pub fn limpar(&self, texto: String) -> String {
        let pares: Vec<(Credencial, &str)> = match self {
            AutorizacaoMcp::Bearer(c) => vec![(c.clone(), Tipo::Bearer.uso())],
            AutorizacaoMcp::Oauth(o) => [Tipo::Acesso, Tipo::Renovacao, Tipo::Cliente]
                .into_iter()
                .filter_map(|t| {
                    credencial(&o.broker, &o.servidor, t)
                        .ok()
                        .flatten()
                        .map(|c| (c, t.uso()))
                })
                .collect(),
        };
        let mut t = texto;
        for (c, uso) in &pares {
            if let Ok(x) = c.abrir(uso) {
                t = x.limpar::<()>(Err(t)).unwrap_err();
            }
        }
        t
    }
}

impl Oauth {
    async fn renovar(&self) -> Result<(), String> {
        let renovacao = credencial(&self.broker, &self.servidor, Tipo::Renovacao)?
            .ok_or("refresh token sumiu do broker: rode `phxclaw mcp login` de novo")?
            .abrir("renovacao")?;
        let cliente = match credencial(&self.broker, &self.servidor, Tipo::Cliente)? {
            Some(c) => Some(c.abrir("cliente")?),
            None if self.cfg.segredo_cliente => {
                return Err("segredo do cliente sumiu do broker".into());
            }
            None => None,
        };
        let r = pedir_token(
            &self.cfg,
            vec![
                ("grant_type".into(), "refresh_token".into()),
                ("refresh_token".into(), renovacao.expor().to_string()),
            ],
            cliente.as_ref().map(|c| c.expor()),
        )
        .await?;
        if let Some(nova) = r.renovacao {
            guardar(&self.broker, &self.servidor, Tipo::Renovacao, nova)?;
        }
        guardar(&self.broker, &self.servidor, Tipo::Acesso, r.acesso)?;
        *self.vence.lock().await = Some(Instant::now() + r.validade);
        Ok(())
    }

    async fn cabecalho(&self) -> Result<String, String> {
        let vencido = {
            let v = self.vence.lock().await;
            v.is_none_or(|v| Instant::now() + FOLGA_DE_VENCIMENTO >= v)
        };
        if vencido {
            self.renovar().await?;
        }
        let c = credencial(&self.broker, &self.servidor, Tipo::Acesso)?
            .ok_or("token de acesso sumiu do broker")?;
        let x = c.abrir("acesso")?;
        Ok(format!("Bearer {}", x.expor()))
    }
}

impl AuthorizationSource for AutorizacaoMcp {
    fn authorization(
        &self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>> {
        Box::pin(async move {
            match self {
                AutorizacaoMcp::Bearer(c) => c.com("bearer", |t| Ok(format!("Bearer {t}"))),
                AutorizacaoMcp::Oauth(o) => o.cabecalho().await,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vetor do apendice B da RFC 7636: confere o S256 contra a norma, nao contra nos.
    #[test]
    fn desafio_s256_bate_com_o_vetor_da_rfc_7636() {
        assert_eq!(
            desafio("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let v = aleatorio().unwrap();
        assert_eq!(v.len(), 43);
        assert_ne!(v, aleatorio().unwrap());
    }
}
