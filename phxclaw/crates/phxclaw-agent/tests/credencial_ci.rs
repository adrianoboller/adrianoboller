//! Credencial e CI (SP000010) e o TLS de soquete dos canais (SP000013), contra servidores
//! FALSOS locais: MCP que exige Bearer (Linear), OAuth 2.0 + PKCE com refresh (Google
//! Workspace), MET Norway com `Expires`/`If-Modified-Since`, e IRC, XMPP (STARTTLS) e IMAP
//! por TLS com certificado proprio (tests/dados/tls, CA de teste valida ate 2126).
//!
//! A prova real do clima (api.met.no de verdade) e `clima_real_do_met_norway`, `#[ignore]`:
//! roda com `--ignored`, porque a suite nao pode depender da rede de fora.

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, Uri};
use phxclaw_agent::canais::Provedor;
use phxclaw_agent::canais::caixa::Caixa;
use phxclaw_agent::canais::http::Credencial;
use phxclaw_agent::canais::tls::Tls;
use phxclaw_agent::canais::{broker_em, guardar_do_canal};
use phxclaw_agent::mcp::{ConfigMcp, carregar_em};
use phxclaw_agent::oauth;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use phxclaw_secret_broker::{SecretBroker, SecretValue};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BEARER: &str = "lin_api_SEGREDO_DO_LINEAR_0123456789";
const SEGREDO_CLIENTE: &str = "GOCSPX-SEGREDO-DO-CLIENTE-OAUTH";
const RENOVACAO: &str = "1//RENOVACAO-QUE-NAO-PODE-VAZAR";

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-credci-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ctx(d: &Path) -> ToolContext {
    ToolContext {
        task_id: "t1".into(),
        workdir: d.to_path_buf(),
        timeout: Duration::from_secs(20),
    }
}

/// Todo arquivo sob `dir` que tem `segredo` em texto puro.
fn arquivos_com(dir: &Path, segredo: &str) -> Vec<PathBuf> {
    let mut achados = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if std::fs::read(&p)
                .map(|b| String::from_utf8_lossy(&b).contains(segredo))
                .unwrap_or(false)
            {
                achados.push(p);
            }
        }
    }
    achados
}

fn ferramenta<'a>(tools: &'a [Arc<dyn Tool>], nome: &str) -> &'a Arc<dyn Tool> {
    tools
        .iter()
        .find(|t| t.spec().name == nome)
        .unwrap_or_else(|| panic!("sem {nome}"))
}

async fn texto(t: &Arc<dyn Tool>, d: &Path) -> Result<String, ToolError> {
    t.run(json!({}), &ctx(d)).await.map(|o| o.content)
}

// ------------------------------------------------------------------ servidor MCP + OAuth falso

#[derive(Default)]
struct Estado {
    /// O `Authorization` que o MCP aceita; qualquer outro (ou nenhum) e 401.
    aceito: String,
    /// Cada `Authorization` que chegou ao MCP, na ordem.
    vistos: Vec<Option<String>>,
    /// OAuth: o que o /authorize recebeu.
    autorizacao: BTreeMap<String, String>,
    /// O refresh token que o /token aceita.
    renovacao: String,
    renovacoes: usize,
}

type Compartilhado = Arc<Mutex<Estado>>;

fn rpc(id: &Value, r: Value) -> (StatusCode, String) {
    (
        StatusCode::OK,
        json!({"jsonrpc": "2.0", "id": id, "result": r}).to_string(),
    )
}

fn mcp(e: &Compartilhado, cab: &HeaderMap, corpo: &[u8]) -> (StatusCode, String) {
    let auth = cab
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let mut g = e.lock().unwrap();
    g.vistos.push(auth.clone());
    if auth.as_deref() != Some(g.aceito.as_str()) {
        return (StatusCode::UNAUTHORIZED, String::new());
    }
    drop(g);
    let v: Value = serde_json::from_slice(corpo).unwrap();
    let id = v.get("id").cloned().unwrap_or(Value::Null);
    let auth = auth.unwrap_or_default();
    match v["method"].as_str().unwrap_or("") {
        "initialize" => rpc(
            &id,
            json!({"protocolVersion": "2025-06-18", "capabilities": {"tools": {}},
                   "serverInfo": {"name": "falso", "version": "1"}}),
        ),
        "notifications/initialized" => (StatusCode::ACCEPTED, String::new()),
        "tools/list" => rpc(
            &id,
            json!({"tools": [
                {"name": "list_issues", "inputSchema": {"type": "object"}},
                {"name": "eco", "inputSchema": {"type": "object"}},
                {"name": "eco_erro", "inputSchema": {"type": "object"}}
            ]}),
        ),
        "tools/call" => match v.pointer("/params/name").and_then(Value::as_str) {
            Some("list_issues") => rpc(
                &id,
                json!({"content": [{"type": "text", "text": "LIN-1 aberto"}]}),
            ),
            // Servidor de terceiro que ecoa o cabecalho: o token nao pode voltar ao modelo.
            Some("eco") => rpc(
                &id,
                json!({"content": [{"type": "text", "text": format!("recebi {auth}")}]}),
            ),
            _ => (
                StatusCode::OK,
                json!({"jsonrpc": "2.0", "id": id,
                       "error": {"code": -32000, "message": format!("recusado: {auth}")}})
                .to_string(),
            ),
        },
        _ => (StatusCode::NOT_FOUND, String::new()),
    }
}

/// O S256 do RFC 7636 calculado AQUI, e nao pelo `oauth::desafio` do agente: o servidor
/// falso que confere com a funcao de quem ele confere aceita qualquer erro dela.
fn s256(verificador: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verificador.as_bytes()))
}

fn token(e: &Compartilhado, corpo: &[u8]) -> (StatusCode, String) {
    let f: BTreeMap<String, String> = url::form_urlencoded::parse(corpo).into_owned().collect();
    let mut g = e.lock().unwrap();
    // Servidor de token que ecoa o pedido no erro: o refresh token e o segredo do cliente
    // nao podem sair dali no texto do agente.
    let recusa = |f: &BTreeMap<String, String>| {
        (
            StatusCode::BAD_REQUEST,
            json!({"error": "invalid_grant", "eco": format!("{f:?}")}).to_string(),
        )
    };
    if f.get("client_id").map(String::as_str) != Some("cli-123.apps")
        || f.get("client_secret").map(String::as_str) != Some(SEGREDO_CLIENTE)
    {
        return recusa(&f);
    }
    match f.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            // A conferencia do PKCE, como o servidor de verdade faz: SHA-256 do verificador
            // em base64url tem de ser o desafio que veio no /authorize.
            let desafio = g.autorizacao.get("code_challenge").cloned();
            let ok = f.get("code").map(String::as_str) == Some("COD-1")
                && f.get("redirect_uri") == g.autorizacao.get("redirect_uri")
                && f.get("code_verifier").map(|v| s256(v)) == desafio;
            if !ok {
                return recusa(&f);
            }
            g.aceito = "Bearer ya29.ACESSO-0".into();
            g.renovacao = RENOVACAO.into();
            (
                StatusCode::OK,
                json!({"access_token": "ya29.ACESSO-0", "refresh_token": RENOVACAO,
                       "expires_in": 3599, "token_type": "Bearer"})
                .to_string(),
            )
        }
        Some("refresh_token") => {
            if f.get("refresh_token") != Some(&g.renovacao) {
                return recusa(&f);
            }
            g.renovacoes += 1;
            let novo = format!("ya29.ACESSO-{}", g.renovacoes);
            g.aceito = format!("Bearer {novo}");
            (
                StatusCode::OK,
                json!({"access_token": novo, "expires_in": 3599, "token_type": "Bearer"})
                    .to_string(),
            )
        }
        _ => recusa(&f),
    }
}

/// Um servidor so com o MCP (`/mcp`), o /authorize e o /token, dividindo o estado.
async fn servidor_falso() -> (String, Compartilhado) {
    let e: Compartilhado = Arc::default();
    let e2 = e.clone();
    let app = axum::Router::new().fallback(move |u: Uri, cab: HeaderMap, corpo: Bytes| {
        let e = e2.clone();
        async move {
            let (status, corpo, local) = match u.path() {
                "/mcp" => {
                    let (s, c) = mcp(&e, &cab, &corpo);
                    (s, c, None)
                }
                "/token" => {
                    let (s, c) = token(&e, &corpo);
                    (s, c, None)
                }
                "/authorize" => {
                    let q: BTreeMap<String, String> =
                        url::form_urlencoded::parse(u.query().unwrap_or("").as_bytes())
                            .into_owned()
                            .collect();
                    let destino = format!(
                        "{}?code=COD-1&state={}",
                        q["redirect_uri"],
                        url::form_urlencoded::byte_serialize(q["state"].as_bytes())
                            .collect::<String>()
                    );
                    e.lock().unwrap().autorizacao = q;
                    (StatusCode::FOUND, String::new(), Some(destino))
                }
                _ => (StatusCode::NOT_FOUND, String::new(), None),
            };
            let mut r = axum::response::Response::builder()
                .status(status)
                .header("content-type", "application/json");
            if let Some(l) = local {
                r = r.header("location", l);
            }
            r.body(axum::body::Body::from(corpo)).unwrap()
        }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, e)
}

fn config(dir: &Path, servidores: Value) -> PathBuf {
    let p = dir.join("mcp.json");
    std::fs::write(&p, json!({"servidores": servidores}).to_string()).unwrap();
    p
}

/// `carregar_em` e sincrono e sobe um runtime proprio: fora do runtime do teste.
async fn carregar(cfg: PathBuf, raiz: PathBuf) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    tokio::task::spawn_blocking(move || carregar_em(&cfg, Some(&raiz)))
        .await
        .unwrap()
}

// ------------------------------------------------------------------ linear (Bearer)

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn linear_pelo_mcp_oficial_com_bearer_do_broker() {
    let (base, e) = servidor_falso().await;
    e.lock().unwrap().aceito = format!("Bearer {BEARER}");
    let raiz = tmp("linear");
    // O preset poe o Bearer; a URL declarada vale mais que a do preset (o servidor falso).
    let cfg = config(
        &raiz,
        json!([
            {"nome": "linear", "preset": "linear", "url": format!("{base}/mcp")},
            {"nome": "sem_credencial", "url": format!("{base}/mcp")}
        ]),
    );
    let declarado: ConfigMcp =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    let resolvida = declarado.servidores[0].resolvida().unwrap();
    assert!(matches!(
        resolvida.auth,
        Some(phxclaw_agent::mcp::AuthDeclarada::Bearer)
    ));

    // Antes do `phxclaw mcp token`: o servidor nao sobe e o aviso diz o comando. E o
    // declarado sem credencial bate no 401 do servidor falso -- ele exige mesmo o Bearer.
    let (tools, avisos) = carregar(cfg.clone(), raiz.clone()).await;
    assert!(tools.is_empty(), "{avisos:?}");
    assert!(
        avisos
            .iter()
            .any(|a| a.contains("phxclaw mcp token linear")),
        "{avisos:?}"
    );
    assert!(
        avisos
            .iter()
            .any(|a| a.contains("sem_credencial") && a.contains("401")),
        "{avisos:?}"
    );
    assert_eq!(
        e.lock().unwrap().vistos,
        vec![None],
        "sem Bearer: um pedido, sem cabecalho"
    );

    let alvo = oauth::Alvo::novo("linear", &format!("{base}/mcp")).unwrap();
    oauth::guardar_bearer(&raiz, &alvo, SecretValue::new(BEARER.into())).unwrap();
    let (tools, avisos) = carregar(cfg, raiz.clone()).await;
    assert!(
        tools
            .iter()
            .any(|t| t.spec().name == "mcp__linear__list_issues"),
        "{avisos:?}"
    );
    assert_eq!(
        ferramenta(&tools, "mcp__linear__list_issues").capability(),
        "mcp.linear"
    );
    let r = texto(ferramenta(&tools, "mcp__linear__list_issues"), &raiz)
        .await
        .unwrap();
    assert_eq!(r, "LIN-1 aberto");
    assert_eq!(
        e.lock().unwrap().vistos.last().unwrap().as_deref(),
        Some(format!("Bearer {BEARER}").as_str())
    );

    // Eco no resultado e no erro: o token sai dos dois.
    let r = texto(ferramenta(&tools, "mcp__linear__eco"), &raiz)
        .await
        .unwrap();
    assert!(!r.contains(BEARER) && r.contains("REDACTED"), "{r}");
    let erro = texto(ferramenta(&tools, "mcp__linear__eco_erro"), &raiz)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        !erro.contains(BEARER) && erro.contains("recusado"),
        "{erro}"
    );

    // Bearer recusado depois (chave revogada no Linear): 401 sobe, sem repetir o pedido --
    // nao ha o que renovar num Bearer fixo.
    let antes = e.lock().unwrap().vistos.len();
    e.lock().unwrap().aceito = "Bearer outra".into();
    for t in &tools {
        t.finish("t1").await;
    }
    let erro = texto(ferramenta(&tools, "mcp__linear__list_issues"), &raiz)
        .await
        .unwrap_err()
        .to_string();
    assert!(erro.contains("401") && !erro.contains(BEARER), "{erro}");
    assert_eq!(e.lock().unwrap().vistos.len(), antes + 1);
    assert!(arquivos_com(&raiz, BEARER).is_empty());
}

// ------------------------------------------------------------------ Google (OAuth + PKCE)

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn google_workspace_por_oauth_pkce_com_refresh_no_broker() {
    let (base, e) = servidor_falso().await;
    let raiz = tmp("google");
    let cfg = config(
        &raiz,
        json!([{
            "nome": "gmail", "preset": "gmail", "url": format!("{base}/mcp"),
            "auth": {"tipo": "oauth", "cliente_id": "cli-123.apps",
                     "autorizacao": format!("{base}/authorize"), "token": format!("{base}/token")}
        }]),
    );
    let declarado: ConfigMcp =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    let Some(phxclaw_agent::mcp::AuthDeclarada::Oauth(o)) =
        declarado.servidores[0].resolvida().unwrap().auth
    else {
        panic!("o preset gmail e OAuth");
    };
    assert!(
        o.segredo_cliente,
        "o preset do Google exige o segredo do cliente"
    );

    let (tools, avisos) = carregar(cfg.clone(), raiz.clone()).await;
    assert!(tools.is_empty());
    assert!(
        avisos.iter().any(|a| a.contains("phxclaw mcp login gmail")),
        "{avisos:?}"
    );

    // O navegador: segue o 302 do /authorize ate o loopback do agente.
    let abriu: Arc<Mutex<Option<String>>> = Arc::default();
    let a2 = abriu.clone();
    let alvo = oauth::Alvo::novo("gmail", &format!("{base}/mcp")).unwrap();
    oauth::login(
        &raiz,
        &alvo,
        &o,
        Some(SecretValue::new(SEGREDO_CLIENTE.into())),
        Duration::from_secs(20),
        move |url| {
            *a2.lock().unwrap() = Some(url.to_string());
            let url = url.to_string();
            std::thread::spawn(move || {
                let r = reqwest::blocking::get(url).unwrap();
                assert_eq!(r.status(), 200);
            });
        },
    )
    .await
    .unwrap();
    let q = e.lock().unwrap().autorizacao.clone();
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["code_challenge"].len(), 43);
    assert_eq!(q["access_type"], "offline");
    assert_eq!(q["scope"], "https://www.googleapis.com/auth/gmail.readonly");
    assert!(q["redirect_uri"].starts_with("http://127.0.0.1:"));
    assert!(q["redirect_uri"].ends_with("/callback"));
    assert!(
        !abriu
            .lock()
            .unwrap()
            .clone()
            .unwrap()
            .contains(SEGREDO_CLIENTE)
    );

    // A descoberta NAO renova: o vencimento e do (servidor, endpoint) no processo (M3), e o
    // login que acabou de trocar o codigo ja o deixou escrito. Processo novo, que nao sabe
    // quando o acesso guardado vence, renova na primeira chamada -- e o que
    // `renovacao_do_oauth_e_uma_so_entre_duas_instancias_do_mesmo_alvo` prova.
    let (tools, avisos) = carregar(cfg, raiz.clone()).await;
    let lista = ferramenta(&tools, "mcp__gmail__list_issues");
    assert!(avisos.is_empty(), "{avisos:?}");
    assert_eq!(e.lock().unwrap().renovacoes, 0);
    let r = texto(lista, &raiz).await;
    assert_eq!(
        r.as_deref().ok(),
        Some("LIN-1 aberto"),
        "{r:?} {:?} {}",
        e.lock().unwrap().vistos,
        e.lock().unwrap().aceito
    );
    assert_eq!(e.lock().unwrap().renovacoes, 0, "acesso valido nao renova");

    // O servidor revoga o acesso antes do prazo: 401 -> renova UMA vez -> repete.
    e.lock().unwrap().aceito = "Bearer revogado".into();
    for t in &tools {
        t.finish("t1").await;
    }
    assert_eq!(texto(lista, &raiz).await.unwrap(), "LIN-1 aberto");
    assert_eq!(e.lock().unwrap().renovacoes, 1);
    assert_eq!(
        e.lock().unwrap().vistos.last().unwrap().as_deref(),
        Some("Bearer ya29.ACESSO-1")
    );

    // Eco do acesso no resultado: sai limpo.
    let r = texto(ferramenta(&tools, "mcp__gmail__eco"), &raiz)
        .await
        .unwrap();
    assert!(!r.contains("ya29.ACESSO") && r.contains("REDACTED"), "{r}");

    // Refresh token revogado no servidor (que ecoa o pedido no erro): o erro diz o
    // `invalid_grant` e nao carrega nenhum dos tres segredos.
    e.lock().unwrap().renovacao = "outro".into();
    e.lock().unwrap().aceito = "Bearer revogado".into();
    for t in &tools {
        t.finish("t1").await;
    }
    let erro = texto(lista, &raiz).await.unwrap_err().to_string();
    assert!(erro.contains("invalid_grant"), "{erro}");
    for s in [RENOVACAO, SEGREDO_CLIENTE, "ya29.ACESSO"] {
        assert!(!erro.contains(s), "{s} vazou: {erro}");
    }
    for s in [RENOVACAO, SEGREDO_CLIENTE, "ya29.ACESSO-1"] {
        assert!(arquivos_com(&raiz, s).is_empty(), "{s} em disco");
    }
}

/// M3: duas instancias do MESMO (servidor, endpoint) no processo, pedindo o cabecalho ao
/// mesmo tempo com o acesso vencido, renovam UMA vez. Com a trava solta entre a conferencia
/// e a renovacao (o defeito), as duas viam «vencido» e iam as duas ao /token -- e com
/// rotacao de refresh token a segunda invalidaria o primeiro.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn renovacao_do_oauth_e_uma_so_entre_duas_instancias_do_mesmo_alvo() {
    use phxclaw_mcp_lsp_runtime::AuthorizationSource;
    let (base, e) = servidor_falso().await;
    let raiz = tmp("oauth-m3");
    let cfg = oauth::ConfigOauth {
        autorizacao: format!("{base}/authorize"),
        token: format!("{base}/token"),
        cliente_id: "cli-123.apps".into(),
        segredo_cliente: true,
        ..Default::default()
    };
    let alvo = oauth::Alvo::novo("gmail_m3", &format!("{base}/mcp")).unwrap();
    oauth::login(
        &raiz,
        &alvo,
        &cfg,
        Some(SecretValue::new(SEGREDO_CLIENTE.into())),
        Duration::from_secs(20),
        |url| {
            let url = url.to_string();
            std::thread::spawn(move || {
                reqwest::blocking::get(url).unwrap();
            });
        },
    )
    .await
    .unwrap();
    assert_eq!(e.lock().unwrap().renovacoes, 0);

    let a = oauth::AutorizacaoMcp::da_pasta(&raiz, &alvo, Some(&cfg)).unwrap();
    let b = oauth::AutorizacaoMcp::da_pasta(&raiz, &alvo, Some(&cfg)).unwrap();
    // O 401 numa instancia vence o acesso para as duas: o vencimento e do alvo, nao da
    // instancia.
    assert!(a.invalidar().await);
    let (ca, cb) = tokio::join!(a.authorization(), b.authorization());
    assert_eq!(ca.as_deref(), Ok("Bearer ya29.ACESSO-1"));
    assert_eq!(cb.as_deref(), Ok("Bearer ya29.ACESSO-1"));
    assert_eq!(
        e.lock().unwrap().renovacoes,
        1,
        "duas instancias do mesmo alvo renovam uma vez so"
    );
    // Valido: nenhuma das duas renova de novo.
    let (ca, cb) = tokio::join!(a.authorization(), b.authorization());
    assert_eq!(ca, cb);
    assert_eq!(e.lock().unwrap().renovacoes, 1);
}

/// M4: o segredo e de (servidor, endpoint). O Bearer guardado quando `linear` apontava
/// para `/mcp-antigo` nao sobe quando a declaracao passa a apontar para `/mcp`: o servidor
/// fica sem credencial e o aviso pede o `phxclaw mcp token` de novo. Com so o nome na
/// chave (o defeito), o token do endpoint antigo ia para o novo.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bearer_guardado_para_outro_endpoint_nao_sobe_para_o_servidor_renomeado() {
    let (base, e) = servidor_falso().await;
    e.lock().unwrap().aceito = format!("Bearer {BEARER}");
    let raiz = tmp("linear-m4");
    let antigo = oauth::Alvo::novo("linear", &format!("{base}/mcp-antigo")).unwrap();
    oauth::guardar_bearer(&raiz, &antigo, SecretValue::new(BEARER.into())).unwrap();

    let cfg = config(
        &raiz,
        json!([{"nome": "linear", "preset": "linear", "url": format!("{base}/mcp")}]),
    );
    let (tools, avisos) = carregar(cfg.clone(), raiz.clone()).await;
    assert!(tools.is_empty(), "{avisos:?}");
    assert!(
        avisos
            .iter()
            .any(|a| a.contains("phxclaw mcp token linear")),
        "{avisos:?}"
    );
    assert!(
        e.lock().unwrap().vistos.is_empty(),
        "sem credencial para este endpoint, nenhum pedido sai: {:?}",
        e.lock().unwrap().vistos
    );

    // Guardado para o endpoint declarado, sobe. E a forma canonica da URL nao duplica.
    let novo = oauth::Alvo::novo(
        "linear",
        &format!("{}/mcp", base.replacen("http", "HTTP", 1)),
    )
    .unwrap();
    assert_eq!(
        novo.endpoint,
        antigo.endpoint.replace("/mcp-antigo", "/mcp")
    );
    oauth::guardar_bearer(&raiz, &novo, SecretValue::new(BEARER.into())).unwrap();
    let (tools, avisos) = carregar(cfg, raiz.clone()).await;
    assert!(
        tools
            .iter()
            .any(|t| t.spec().name == "mcp__linear__list_issues"),
        "{avisos:?}"
    );
}

#[tokio::test]
async fn loopback_do_oauth_so_aceita_o_state_combinado() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let end = l.local_addr().unwrap();
    let pedido = |caminho: &'static str| async move {
        let mut s = tokio::net::TcpStream::connect(end).await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        s.write_all(format!("GET {caminho} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).await.unwrap();
        r.lines().next().unwrap_or("").to_string()
    };
    let espera = tokio::spawn(async move {
        oauth::esperar_codigo(l, "STATE-BOM", Duration::from_secs(10)).await
    });
    assert!(
        pedido("/callback?code=ROUBADO&state=OUTRO")
            .await
            .contains("400")
    );
    assert!(pedido("/favicon.ico").await.contains("404"));
    assert!(
        pedido("/callback?code=CERTO&state=STATE-BOM")
            .await
            .contains("200")
    );
    assert_eq!(espera.await.unwrap().unwrap(), "CERTO");

    // Negada pelo usuario: erro com o motivo, sem esperar o prazo.
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let end = l.local_addr().unwrap();
    let espera =
        tokio::spawn(async move { oauth::esperar_codigo(l, "S", Duration::from_secs(10)).await });
    let mut s = tokio::net::TcpStream::connect(end).await.unwrap();
    use tokio::io::AsyncWriteExt;
    s.write_all(b"GET /callback?error=access_denied&state=S HTTP/1.1\r\n\r\n")
        .await
        .unwrap();
    let e = espera.await.unwrap().unwrap_err();
    assert!(e.contains("access_denied"), "{e}");
}

// ------------------------------------------------------------------ clima (MET Norway)

#[derive(Default)]
struct Met {
    pedidos: Vec<(String, Option<String>, Option<String>)>,
}

fn previsao() -> Value {
    json!({"properties": {"meta": {"updated_at": "2026-10-01T10:00:00Z",
    "units": {"air_temperature": "celsius", "wind_speed": "m/s"}},
    "timeseries": [
        {"time": "2026-10-01T11:00:00Z", "data": {"instant": {"details":
            {"air_temperature": 21.5, "wind_speed": 3.1, "relative_humidity": 70.0}},
            "next_1_hours": {"summary": {"symbol_code": "partlycloudy_day"},
                             "details": {"precipitation_amount": 0.0}}}},
        {"time": "2026-10-01T12:00:00Z", "data": {"instant": {"details":
            {"air_temperature": 22.0, "wind_speed": 3.4}}}}
    ]}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clima_respeita_expires_e_if_modified_since_com_atribuicao() {
    use phxclaw_agent::clima::{ATRIBUICAO, WeatherTool};
    let m: Arc<Mutex<Met>> = Arc::default();
    let m2 = m.clone();
    let app = axum::Router::new().fallback(move |u: Uri, cab: HeaderMap| {
        let m = m2.clone();
        async move {
            let h = |n: &str| cab.get(n).and_then(|v| v.to_str().ok()).map(str::to_string);
            let q = u.query().unwrap_or("").to_string();
            m.lock()
                .unwrap()
                .pedidos
                .push((q.clone(), h("user-agent"), h("if-modified-since")));
            let modificado = "Wed, 01 Oct 2026 10:00:00 GMT";
            // lat=10: vale por uma hora; lat=20: ja venceu (o proximo pedido pergunta).
            let expira = if q.contains("lat=10") {
                (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc2822()
            } else {
                "Wed, 01 Oct 2025 10:00:00 GMT".into()
            };
            let mut r = axum::response::Response::builder()
                .header("expires", expira)
                .header("last-modified", modificado)
                .header("content-type", "application/json");
            if h("if-modified-since").as_deref() == Some(modificado) {
                r = r.status(304);
                return r.body(axum::body::Body::empty()).unwrap();
            }
            r.status(200)
                .body(axum::body::Body::from(previsao().to_string()))
                .unwrap()
        }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!(
        "http://{}/weatherapi/locationforecast/2.0",
        l.local_addr().unwrap()
    );
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let t: Arc<dyn Tool> = Arc::new(WeatherTool::novo(&base).unwrap());
    assert_eq!(t.capability(), "weather.read");
    let d = tmp("clima");
    let chamar = |lat: f64| {
        let t = t.clone();
        let d = d.clone();
        async move {
            let o = t
                .run(
                    json!({"lat": lat, "lon": 10.123_456_7, "hours": 1}),
                    &ctx(&d),
                )
                .await
                .unwrap();
            serde_json::from_str::<Value>(&o.content).unwrap()
        }
    };
    let v = chamar(10.0).await;
    assert_eq!(v["atribuicao"], ATRIBUICAO);
    assert_eq!(v["horas"].as_array().unwrap().len(), 1);
    assert_eq!(v["horas"][0]["temperatura"], 21.5);
    assert_eq!(v["horas"][0]["simbolo"], "partlycloudy_day");
    // Antes do Expires: nada sai pela rede.
    chamar(10.0).await;
    {
        let g = m.lock().unwrap();
        assert_eq!(g.pedidos.len(), 1, "o Expires nao foi respeitado");
        let (q, agente, desde) = &g.pedidos[0];
        assert!(q.contains("lon=10.1234") && !q.contains("10.12345"), "{q}");
        assert!(
            agente.as_deref().unwrap_or("").starts_with("PhxClaw/")
                && agente.as_deref().unwrap_or("").contains("http"),
            "{agente:?}"
        );
        assert!(desde.is_none());
    }
    // Vencido: pergunta com If-Modified-Since e reaproveita o corpo no 304.
    chamar(20.0).await;
    let v = chamar(20.0).await;
    assert_eq!(v["horas"][0]["temperatura"], 21.5);
    assert_eq!(v["atribuicao"], ATRIBUICAO);
    let g = m.lock().unwrap();
    assert_eq!(g.pedidos.len(), 3);
    assert_eq!(
        g.pedidos[2].2.as_deref(),
        Some("Wed, 01 Oct 2026 10:00:00 GMT")
    );
}

/// Prova REAL: api.met.no de verdade (`cargo test ... -- --ignored clima_real`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn clima_real_do_met_norway() {
    use phxclaw_agent::clima::{ATRIBUICAO, BASE_PADRAO, WeatherTool};
    let t = WeatherTool::novo(BASE_PADRAO).unwrap();
    let d = tmp("clima-real");
    // Blumenau.
    let o = t
        .run(
            json!({"lat": -26.9194, "lon": -49.0661, "hours": 3}),
            &ctx(&d),
        )
        .await
        .unwrap();
    let v: Value = serde_json::from_str(&o.content).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&v).unwrap());
    assert_eq!(v["atribuicao"], ATRIBUICAO);
    assert_eq!(v["horas"].as_array().unwrap().len(), 3);
    assert!(v["horas"][0]["temperatura"].is_number(), "{v}");
}

// ------------------------------------------------------------------ TLS dos canais

const CA: &[u8] = include_bytes!("dados/tls/ca.pem");
const OUTRA_CA: &[u8] = include_bytes!("dados/tls/outra.pem");
const SENHA: &str = "SENHA-DE-CANAL-QUE-NAO-PODE-VAZAR";

fn config_servidor() -> Arc<rustls::ServerConfig> {
    use rustls_pki_types::pem::PemObject;
    use rustls_pki_types::{CertificateDer, PrivateKeyDer};
    let certs: Vec<_> = CertificateDer::pem_slice_iter(include_bytes!("dados/tls/srv.pem"))
        .map(Result::unwrap)
        .collect();
    let chave = PrivateKeyDer::from_pem_slice(include_bytes!("dados/tls/srv.key")).unwrap();
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, chave)
        .unwrap(),
    )
}

type FluxoTls = rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>;

fn tls_do_servidor(s: std::net::TcpStream) -> Option<FluxoTls> {
    let mut c = rustls::ServerConnection::new(config_servidor()).unwrap();
    let mut s = s;
    while c.is_handshaking() {
        // Cliente que recusou o certificado fecha no meio: o servidor so desiste.
        c.complete_io(&mut s).ok()?;
    }
    Some(rustls::StreamOwned::new(c, s))
}

fn cred(b: &Arc<SecretBroker>, canal: &str) -> Credencial {
    let id = guardar_do_canal(
        b,
        &format!("{canal}-senha"),
        canal,
        SecretValue::new(SENHA.into()),
    )
    .unwrap();
    Credencial::nova(b.clone(), id, canal)
}

async fn bloq<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

/// IRC por TLS: registra (001), entrega uma PRIVMSG e um PING, e guarda o que o cliente
/// mandou (PASS inclusive, que so trafega cifrado).
fn irc_tls() -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = format!("localhost:{}", l.local_addr().unwrap().port());
    let linhas: Arc<Mutex<Vec<String>>> = Arc::default();
    let l2 = linhas.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let Some(t) = tls_do_servidor(s) else {
                continue;
            };
            let l2 = l2.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(t);
                let mut linha = String::new();
                while r.read_line(&mut linha).unwrap_or(0) > 0 {
                    let x = linha.trim_end().to_string();
                    linha.clear();
                    l2.lock().unwrap().push(x.clone());
                    let w = r.get_mut();
                    if x.starts_with("USER") {
                        w.write_all(b":srv 001 bot :oi\r\n").unwrap();
                    } else if x.starts_with("JOIN") {
                        w.write_all(b"PING :srv\r\n:ana!a@h PRIVMSG #sala :ola por tls\r\n")
                            .unwrap();
                    }
                    w.flush().unwrap();
                }
            });
        }
    });
    (end, linhas)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn irc_e_twitch_por_tls_com_certificado_conferido() {
    use phxclaw_agent::canais::irc::{Config, Irc};
    for nome in ["irc", "twitch"] {
        let dir = tmp("irc-tls");
        let b = broker_em(&dir).unwrap();
        let (end, linhas) = irc_tls();
        let irc = Arc::new(Irc::novo(
            Config {
                nome,
                endereco: end.clone(),
                nick: "bot".into(),
                senha: Some(cred(&b, nome)),
                salas: vec!["#sala".into()],
                tls: Some(Tls::com_ca_pem(CA).unwrap()),
            },
            Caixa::abrir(dir.join("c.jsonl")).unwrap(),
        ));
        let x = irc.clone();
        let lote = bloq(move || x.receber(None, 2)).await.unwrap();
        let textos: Vec<_> = lote
            .iter()
            .filter_map(|e| e.mensagem.as_ref()?.texto.clone())
            .collect();
        assert_eq!(textos, vec!["ola por tls".to_string()], "{nome}");
        let x = irc.clone();
        bloq(move || x.enviar("#sala", "resposta cifrada"))
            .await
            .unwrap();
        // O escritor nao fica preso atras do leitor: a resposta chega logo.
        let mut ok = false;
        for _ in 0..50 {
            let l = linhas.lock().unwrap().clone();
            if l.iter().any(|x| x == "PRIVMSG #sala :resposta cifrada")
                && l.iter().any(|x| x == "PONG :srv")
            {
                ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let l = linhas.lock().unwrap().clone();
        assert!(ok, "{nome}: {l:?}");
        assert!(l.contains(&format!("PASS {SENHA}")), "{l:?}");

        // CA errada: o certificado do servidor e recusado ja na conexao, sem senha mandada.
        let (end, linhas) = irc_tls();
        let irc = Irc::novo(
            Config {
                nome,
                endereco: end,
                nick: "bot".into(),
                senha: Some(cred(&b, nome)),
                salas: vec![],
                tls: Some(Tls::com_ca_pem(OUTRA_CA).unwrap()),
            },
            Caixa::abrir(dir.join("d.jsonl")).unwrap(),
        );
        let e = bloq(move || irc.receber(None, 0)).await.unwrap_err();
        assert!(e.contains("tls") && !e.contains(SENHA), "{e}");
        assert!(linhas.lock().unwrap().is_empty());
    }
}

/// XMPP com STARTTLS: anuncia, recebe o pedido, diz `proceed` e sobe o TLS no MESMO TCP.
fn xmpp_starttls(oferece: bool) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = format!("localhost:{}", l.local_addr().unwrap().port());
    let vistos: Arc<Mutex<Vec<String>>> = Arc::default();
    let v2 = vistos.clone();
    std::thread::spawn(move || {
        let Ok((mut s, _)) = l.accept() else { return };
        let ler = |s: &mut dyn Read, marca: &str, v: &Arc<Mutex<Vec<String>>>| {
            let mut buf = String::new();
            let mut b = [0u8; 4096];
            while !buf.contains(marca) {
                let n = s.read(&mut b).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.push_str(&String::from_utf8_lossy(&b[..n]));
            }
            v.lock().unwrap().push(buf);
        };
        ler(&mut s, "version='1.0'>", &v2);
        let starttls = if oferece {
            "<starttls xmlns='urn:ietf:params:xml:ns:xmpp-tls'><required/></starttls>"
        } else {
            ""
        };
        write!(s, "<?xml version='1.0'?><stream:stream from='localhost' id='0' version='1.0' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams'><stream:features>{starttls}<mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>").unwrap();
        if !oferece {
            ler(&mut s, "</auth>", &v2);
            return;
        }
        ler(&mut s, "<starttls", &v2);
        s.write_all(b"<proceed xmlns='urn:ietf:params:xml:ns:xmpp-tls'/>")
            .unwrap();
        let Some(mut t) = tls_do_servidor(s) else {
            return;
        };
        ler(&mut t, "version='1.0'>", &v2);
        t.write_all(b"<stream:stream from='localhost' id='1' version='1.0'><stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>").unwrap();
        ler(&mut t, "</auth>", &v2);
        t.write_all(b"<success xmlns='urn:ietf:params:xml:ns:xmpp-sasl'/>")
            .unwrap();
        ler(&mut t, "version='1.0'>", &v2);
        t.write_all(b"<stream:stream from='localhost' id='2' version='1.0'><stream:features><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'/></stream:features>").unwrap();
        ler(&mut t, "</iq>", &v2);
        t.write_all(b"<iq type='result' id='bind1'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'><jid>agente@localhost/phxclaw</jid></bind></iq>").unwrap();
        ler(&mut t, "<presence/>", &v2);
        t.write_all(b"<message from='ana@localhost/tel' type='chat' id='m1'><body>oi cifrado</body></message>").unwrap();
        t.flush().unwrap();
        ler(&mut t, "</message>", &v2);
        std::thread::sleep(Duration::from_millis(500));
    });
    (end, vistos)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_sobe_starttls_antes_da_senha_e_recusa_quem_nao_oferece() {
    use phxclaw_agent::canais::xmpp::{Config, Xmpp};
    let dir = tmp("xmpp-tls");
    let b = broker_em(&dir).unwrap();
    let (end, vistos) = xmpp_starttls(true);
    let x = Arc::new(Xmpp::novo(
        Config {
            endereco: end,
            jid: "agente@localhost".into(),
            senha: cred(&b, "xmpp"),
            tls: Some(Tls::com_ca_pem(CA).unwrap()),
        },
        Caixa::abrir(dir.join("c.jsonl")).unwrap(),
    ));
    let y = x.clone();
    let lote = bloq(move || y.receber(None, 2)).await.unwrap();
    let textos: Vec<_> = lote
        .iter()
        .filter_map(|e| e.mensagem.as_ref()?.texto.clone())
        .collect();
    assert_eq!(textos, vec!["oi cifrado".to_string()]);
    let y = x.clone();
    bloq(move || y.enviar("ana@localhost", "volta"))
        .await
        .unwrap();
    for _ in 0..30 {
        if vistos.lock().unwrap().iter().any(|v| v.contains("volta")) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let v = vistos.lock().unwrap().clone();
    assert!(v.iter().any(|x| x.contains("<body>volta</body>")), "{v:?}");
    // O que passou em texto claro (os dois primeiros blocos) nao tem a senha do SASL.
    let credencial_plain = SENHA;
    assert!(
        v[..2].iter().all(|x| !x.contains("<auth")),
        "auth antes do TLS: {v:?}"
    );
    assert!(!v.iter().any(|x| x.contains(credencial_plain)));

    // Servidor sem STARTTLS: recusa antes de mandar a senha.
    let (end, vistos) = xmpp_starttls(false);
    let x = Xmpp::novo(
        Config {
            endereco: end,
            jid: "agente@localhost".into(),
            senha: cred(&b, "xmpp"),
            tls: Some(Tls::com_ca_pem(CA).unwrap()),
        },
        Caixa::abrir(dir.join("d.jsonl")).unwrap(),
    );
    let e = bloq(move || x.receber(None, 0)).await.unwrap_err();
    assert!(e.contains("STARTTLS"), "{e}");
    assert!(!vistos.lock().unwrap().iter().any(|v| v.contains("<auth")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn imap_por_tls_na_porta_993() {
    use phxclaw_agent::canais::email::{Config, Email};
    use phxclaw_agent::email::{SmtpConfig, SmtpSecurity};
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = format!("localhost:{}", l.local_addr().unwrap().port());
    let vistos: Arc<Mutex<Vec<String>>> = Arc::default();
    let v2 = vistos.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let Some(mut t) = tls_do_servidor(s) else {
                continue;
            };
            t.write_all(b"* OK IMAP por TLS\r\n").unwrap();
            t.flush().unwrap();
            let mut r = BufReader::new(t);
            let mut linha = String::new();
            while r.read_line(&mut linha).unwrap_or(0) > 0 {
                let (tag, cmd) = linha.trim_end().split_once(' ').unwrap();
                let (tag, cmd) = (tag.to_string(), cmd.to_string());
                linha.clear();
                v2.lock().unwrap().push(cmd.clone());
                let resp = if cmd.starts_with("LOGIN") {
                    format!("{tag} OK\r\n")
                } else if cmd.starts_with("SELECT") {
                    format!("* OK [UIDVALIDITY 7] ok\r\n{tag} OK\r\n")
                } else if cmd == "UID SEARCH ALL" {
                    format!("* SEARCH 4 5\r\n{tag} OK\r\n")
                } else {
                    format!("{tag} OK\r\n")
                };
                r.get_mut().write_all(resp.as_bytes()).unwrap();
                r.get_mut().flush().unwrap();
            }
        }
    });
    let dir = tmp("imap-tls");
    let b = broker_em(&dir).unwrap();
    let e = Email::novo(
        Config {
            endereco: end,
            usuario: "agente".into(),
            senha: cred(&b, "email"),
            smtp_senha: None,
            pasta: "INBOX".into(),
            exigir_dmarc: true,
            tls: Some(Tls::com_ca_pem(CA).unwrap()),
        },
        SmtpConfig {
            host: "127.0.0.1".into(),
            port: 1,
            security: SmtpSecurity::Plain,
            username: None,
            password: None,
            from: "agente@x.org".into(),
            allowed_recipients: vec![],
        },
    );
    let lote = bloq(move || e.receber(None, 0)).await.unwrap();
    assert_eq!(lote[0].cursor.as_deref(), Some("7:6"));
    let v = vistos.lock().unwrap().clone();
    assert!(v[0].starts_with("LOGIN") && v[0].contains(SENHA), "{v:?}");

    // Sem TLS e fora de loopback continua recusado (a regra antiga vale).
    let e = phxclaw_agent::canais::tls::conectar("8.8.8.8:143", None)
        .err()
        .unwrap();
    assert!(e.contains("loopback"), "{e}");
}
