//! O no HTTP generico (`fluxo_http.rs`), o OAuth2 nomeado e o gatilho de poll
//! (`gatilho_poll.rs`), contra um servidor HTTP falso local (axum) -- sem rede externa.
//!
//! Prova real: os testes marcados «RED medido» tiveram o defeito reposto de verdade (linha
//! marcada `// REPOSTO`, recompilada, vista cair pelo motivo certo) e o conserto voltou.

mod comum_fluxo;

use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use comum_fluxo::*;
use phxclaw_agent::api::ApiState;
use phxclaw_agent::fluxo_http::{self, HttpTool};
use phxclaw_agent::fluxos;
use phxclaw_agent::gatilho_poll::{self, Sondagem, Volta};
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext};
use phxclaw_secret_broker::SecretValue;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SEGREDO: &str = "tok-SEGREDO-de-teste-123456";
const CLIENTE: &str = "cliente-SEGREDO-987654";

/// A pasta temporaria de um teste, apagada quando ele termina -- inclusive quando cai (o
/// `Drop` roda no desenrolar do panico). Sem isso cada corrida deixava as suas em /tmp, e
/// eram milhares de `phx-onda3-*`.
struct Pasta(std::path::PathBuf);

impl std::ops::Deref for Pasta {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Sombreia o `tmp` da banca comum so neste arquivo: os outros arquivos de teste a usam
/// como esta.
fn tmp(nome: &str) -> Pasta {
    Pasta(comum_fluxo::tmp(nome))
}

#[derive(Default)]
struct Falso {
    toques: AtomicUsize,
    tokens: AtomicUsize,
    acesso: Mutex<String>,
    lotes: Mutex<Vec<(Instant, Value)>>,
    poll: Mutex<Vec<Value>>,
    vistos: Mutex<Vec<String>>,
}

type Est = Arc<Falso>;

fn auth(h: &HeaderMap) -> String {
    h.get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

async fn servidor() -> (String, Est) {
    let e: Est = Arc::new(Falso::default());
    let app = Router::new()
        .route(
            "/itens",
            get(|State(e): State<Est>| async move {
                e.toques.fetch_add(1, Ordering::SeqCst);
                Json(json!({"data": [{"id": 1}, {"id": 2}]}))
            }),
        )
        .route(
            "/cursor",
            get(|Query(q): Query<BTreeMap<String, String>>| async move {
                let (itens, prox) = match q.get("cursor").map(String::as_str) {
                    None => (json!([{"n": 1}, {"n": 2}]), json!("c2")),
                    Some("c2") => (json!([{"n": 3}]), json!("c3")),
                    Some("c3") => (json!([{"n": 4}]), Value::Null),
                    Some(_) => (json!([]), Value::Null),
                };
                assert_eq!(q.get("fixo").map(String::as_str), Some("1"));
                Json(json!({"itens": itens, "meta": {"proximo": prox}}))
            }),
        )
        .route(
            "/link",
            get(|Query(q): Query<BTreeMap<String, String>>| async move {
                let p: u32 = q.get("p").and_then(|p| p.parse().ok()).unwrap_or(1);
                let mut r = Json(json!([{"p": p}])).into_response();
                if p < 3 {
                    r.headers_mut().insert(
                        header::LINK,
                        format!("</link?p={}>; rel=\"next\", </link?p=3>; rel=\"last\"", p + 1)
                            .parse()
                            .unwrap(),
                    );
                }
                r
            }),
        )
        .route(
            "/lote",
            post(|State(e): State<Est>, Json(v): Json<Value>| async move {
                let n = v.get("itens").and_then(Value::as_array).map_or(0, Vec::len);
                e.lotes.lock().unwrap().push((Instant::now(), v));
                Json(json!({"recebidos": n}))
            }),
        )
        .route(
            "/eco",
            get(|State(e): State<Est>, h: HeaderMap| async move {
                e.toques.fetch_add(1, Ordering::SeqCst);
                // O servidor de terceiro que ecoa o pedido no erro.
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("falhou; recebi authorization: {}", auth(&h)),
                )
            }),
        )
        .route(
            "/eco-ok",
            get(|h: HeaderMap| async move {
                // O servidor que ecoa o pedido no SUCESSO: no corpo, num cabecalho e sem o
                // «Bearer» na frente (a forma que so a tarja pelo valor exato pega).
                let a = auth(&h);
                let token = a.strip_prefix("Bearer ").unwrap_or(&a).to_string();
                (
                    [("x-eco", token.clone())],
                    Json(json!({"recebi": token, "inteiro": a, "nota": format!("o token e {token}.")})),
                )
            }),
        )
        .route(
            "/grande",
            get(|| async { "x".repeat(3 * 1024 * 1024) }),
        )
        .route("/meio", get(|| async { "m".repeat(700 * 1024) }))
        .route(
            "/grande-sem-tamanho",
            get(|| async {
                let pedacos = (0..48).map(|_| Ok::<_, std::io::Error>(Bytes::from(vec![b'y'; 64 * 1024])));
                Body::from_stream(futures_util::stream::iter(pedacos))
            }),
        )
        .route(
            "/token",
            post(
                |State(e): State<Est>, axum::Form(f): axum::Form<BTreeMap<String, String>>| async move {
                    let n = e.tokens.fetch_add(1, Ordering::SeqCst) + 1;
                    let ok = match f.get("grant_type").map(String::as_str) {
                        Some("client_credentials") => {
                            f.get("client_secret").map(String::as_str) == Some(CLIENTE)
                        }
                        Some("refresh_token") => {
                            f.get("refresh_token").map(String::as_str) == Some("renova-SEGREDO-1")
                        }
                        _ => false,
                    };
                    if !ok {
                        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid_grant"})))
                            .into_response();
                    }
                    let acesso = format!("acesso-{n}");
                    *e.acesso.lock().unwrap() = acesso.clone();
                    Json(json!({"access_token": acesso, "expires_in": 3600})).into_response()
                },
            ),
        )
        .route(
            "/protegido",
            get(|State(e): State<Est>, h: HeaderMap| async move {
                let esperado = format!("Bearer {}", e.acesso.lock().unwrap());
                if auth(&h) == esperado || auth(&h) == format!("Bearer {SEGREDO}") {
                    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
                } else {
                    StatusCode::UNAUTHORIZED.into_response()
                }
            }),
        )
        .route(
            "/basico",
            get(|h: HeaderMap| async move {
                use base64::Engine;
                let b = base64::engine::general_purpose::STANDARD
                    .encode(format!("fulano:{SEGREDO}"));
                let ok = auth(&h) == format!("Basic {b}");
                let chave = h.get("x-chave").and_then(|v| v.to_str().ok()) == Some(SEGREDO);
                Json(json!({"basico": ok, "chave": chave}))
            }),
        )
        .route(
            "/poll",
            get(|State(e): State<Est>| async move { Json(Value::Array(e.poll.lock().unwrap().clone())) }),
        )
        .route(
            "/feed",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/rss+xml")],
                    "<rss><channel><item><guid>a</guid><title>A</title></item>\
<item><guid>b</guid><title>B</title></item></channel></rss>",
                )
            }),
        )
        .route(
            "/quem-vem",
            get(|State(e): State<Est>, h: HeaderMap| async move {
                e.toques.fetch_add(1, Ordering::SeqCst);
                e.vistos.lock().unwrap().push(auth(&h));
                Json(json!({"ok": true}))
            }),
        )
        .with_state(e.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, e)
}

/// Servidor que so redireciona (302) para `destino`.
async fn redirecionador(destino: String) -> String {
    redirecionador_com_corpo(destino, 0).await
}

/// O mesmo, com `bytes` de corpo no 302 (o corpo do salto tambem e lido e tambem conta).
async fn redirecionador_com_corpo(destino: String, bytes: usize) -> String {
    let app = Router::new().route(
        "/ir",
        get(move || {
            let d = destino.clone();
            async move {
                Response::builder()
                    .status(StatusCode::FOUND)
                    .header(header::LOCATION, d)
                    .body(Body::from("r".repeat(bytes)))
                    .unwrap()
            }
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

fn config(raiz: &Path, v: Value) {
    std::fs::write(raiz.join(fluxo_http::ARQUIVO), v.to_string()).unwrap();
}

async fn http(raiz: &Path, v: Value) -> Result<Vec<Value>, String> {
    fluxo_http::executar(raiz, &v).await
}

/// O estado da API com a fabrica montando o agente com a ferramenta `http_request` (sobre
/// `raiz`) e o `eco`, com as capacidades `caps`: e por esse agente que o poll faz o pedido.
fn estado_http(raiz: &Path, caps: &'static [&'static str]) -> ApiState {
    let mut s = estado(raiz);
    let (st, r) = (s.store.clone(), raiz.to_path_buf());
    s.factory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(HttpTool { raiz: r.clone() }),
            Arc::new(Eco::default()),
        ];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("feito")])),
            tools,
            AgentConfig::default().grant(caps),
            st.clone(),
        ))
    });
    s
}

/// O agente com a ferramenta `http_request` (e o `eco`) sobre `raiz`.
fn agente(raiz: &Path, caps: &[&str]) -> Agent {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(HttpTool {
            raiz: raiz.to_path_buf(),
        }),
        Arc::new(Eco::default()),
    ];
    Agent::new(
        Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("nada")])),
        tools,
        AgentConfig::default().grant(caps),
        TaskStore::new(raiz.join("tasks")).unwrap(),
    )
}

// ======================================================================== SSRF

/// Por padrao o no recusa loopback, rede privada, link-local e o endereco de metadados de
/// nuvem ANTES de conectar; so a origem escrita em `liberar` passa, e so ela.
///
/// RED medido: em `fluxo_http::Execucao::enviar`, a conferencia `check_url_resolved`
/// trocada por aceitar tudo (`// REPOSTO`): o pedido ao 127.0.0.1 volta com os itens.
#[tokio::test]
async fn ssrf_recusa_interno_por_padrao_e_libera_so_a_origem_listada() {
    let (base, e) = servidor().await;
    let raiz = tmp("http-ssrf");
    for url in [
        format!("{base}/itens"),
        format!("{}/itens", base.replace("127.0.0.1", "localhost")),
        "http://169.254.169.254/latest/meta-data".to_string(),
        "http://[::ffff:127.0.0.1]:1/".to_string(),
        "http://10.0.0.1/".to_string(),
    ] {
        let r = http(&raiz, json!({"url": url})).await;
        let erro = r.expect_err(&url);
        assert!(
            erro.contains("endereco interno") || erro.contains("refused"),
            "{url}: {erro}"
        );
    }
    assert_eq!(
        e.toques.load(Ordering::SeqCst),
        0,
        "o interno recebeu conexao"
    );
    config(&raiz, json!({"liberar": [format!("{base}/")]}));
    let itens = http(
        &raiz,
        json!({"url": format!("{base}/itens"), "itens": "data"}),
    )
    .await
    .unwrap();
    assert_eq!(itens, vec![json!({"id": 1}), json!({"id": 2})]);
    // A liberacao e por origem exata: a mesma maquina em outra porta continua fechada.
    let outra = base.rsplit_once(':').unwrap().0.to_string() + ":1";
    assert!(
        http(&raiz, json!({"url": format!("{outra}/x")}))
            .await
            .is_err()
    );
}

/// O redirecionamento revalida o destino: a origem liberada respondendo 302 para um interno
/// NAO liberado e recusada, e o interno nao recebe conexao.
///
/// RED medido: em `phxclaw_egress_broker::request_checked`, a conferencia `check` so no
/// primeiro salto (`if n == 0`, `// REPOSTO`): o pedido segue o 302 e chega ao interno.
#[tokio::test]
async fn redirecionamento_para_interno_e_recusado_sem_tocar_o_alvo() {
    let (interno, e) = servidor().await;
    let liberado = redirecionador(format!("{interno}/itens")).await;
    let raiz = tmp("http-redir");
    config(&raiz, json!({"liberar": [liberado.clone()]}));
    let erro = http(&raiz, json!({"url": format!("{liberado}/ir")}))
        .await
        .unwrap_err();
    assert!(erro.contains("endereco interno"), "{erro}");
    assert_eq!(
        e.toques.load(Ordering::SeqCst),
        0,
        "o interno recebeu conexao"
    );
    // Liberado o destino tambem, o salto segue.
    config(
        &raiz,
        json!({"liberar": [liberado.clone(), interno.clone()]}),
    );
    let itens = http(
        &raiz,
        json!({"url": format!("{liberado}/ir"), "itens": "data"}),
    )
    .await
    .unwrap();
    assert_eq!(itens.len(), 2);
}

// ======================================================================== teto de bytes

/// O teto de bytes aborta a resposta grande: a que declara o tamanho (recusada sem ler) e a
/// que vem em pedacos sem `Content-Length` (abortada ao passar).
///
/// RED medido: em `phxclaw_http_client::http_request_with`, o `if corpo.len() + pedaco.len()
/// > teto` retirado (`// REPOSTO`): a resposta em pedacos de 3 MiB entra inteira num teto de
/// 1 MiB.
#[tokio::test]
async fn teto_de_bytes_aborta_a_resposta_grande() {
    let (base, _) = servidor().await;
    let raiz = tmp("http-teto");
    config(&raiz, json!({"liberar": [base.clone()]}));
    for rota in ["grande", "grande-sem-tamanho"] {
        let erro = http(
            &raiz,
            json!({"url": format!("{base}/{rota}"), "teto_bytes": 1024 * 1024}),
        )
        .await
        .unwrap_err();
        assert!(erro.contains("teto de 1048576 bytes"), "{rota}: {erro}");
    }
    // Abaixo do teto, passa.
    let itens = http(
        &raiz,
        json!({"url": format!("{base}/grande"), "teto_bytes": 4 * 1024 * 1024}),
    )
    .await
    .unwrap();
    assert_eq!(itens[0].as_str().unwrap().len(), 3 * 1024 * 1024);
    // O teto vale para o no inteiro, somando as paginas.
    let erro = http(
        &raiz,
        json!({"url": format!("{base}/link"), "teto_bytes": 20, "paginacao": {"link": true}}),
    )
    .await
    .unwrap_err();
    assert!(erro.contains("teto"), "{erro}");
}

/// O corpo de cada salto de redirecionamento conta no teto do no: dois corpos de 700 KiB
/// (o do 302 e o do destino) nao cabem num teto de 1 MiB.
///
/// RED medido: em `fluxo_http::Execucao::enviar`, as opcoes do salto sem o contador
/// (`contador: None`, `// REPOSTO`): so o corpo do ultimo salto contava, e o pedido passava.
#[tokio::test]
async fn corpo_de_cada_salto_de_redirecionamento_conta_no_teto() {
    let (base, _) = servidor().await;
    let salto = redirecionador_com_corpo(format!("{base}/meio"), 700 * 1024).await;
    let raiz = tmp("http-teto-saltos");
    config(&raiz, json!({"liberar": [base.clone(), salto.clone()]}));
    let erro = match http(
        &raiz,
        json!({"url": format!("{salto}/ir"), "teto_bytes": 1024 * 1024}),
    )
    .await
    {
        Err(e) => e,
        // Sem o corpo no panico: sao 700 KiB.
        Ok(i) => panic!(
            "os dois corpos passaram do teto e o no aceitou ({} item)",
            i.len()
        ),
    };
    assert!(erro.contains("teto de 1048576 bytes"), "{erro}");
    let itens = http(
        &raiz,
        json!({"url": format!("{salto}/ir"), "teto_bytes": 2 * 1024 * 1024}),
    )
    .await
    .expect("com teto para os dois corpos, passa");
    assert_eq!(itens[0].as_str().map(str::len), Some(700 * 1024));
}

// ======================================================================== credencial

fn guardar(raiz: &Path, nome: &str, valor: &str) {
    fluxo_http::guardar_credencial(raiz, nome, SecretValue::new(valor.into())).unwrap();
}

/// O erro HTTP aparece no relatorio do fluxo -- status, metodo, URL, o comeco do corpo --
/// e o segredo que o servidor ecoou NAO aparece, nem no relatorio nem no `task.json`.
///
/// RED medido: em `fluxo_http::Execucao::enviar`, o erro de status montado sem
/// `self.limpar` (`// REPOSTO`): o Bearer ecoado aparece no relatorio.
#[tokio::test]
async fn erro_http_aparece_no_relatorio_sem_vazar_o_authorization() {
    let (base, e) = servidor().await;
    let raiz = tmp("http-tarja");
    config(
        &raiz,
        json!({"liberar": [base.clone()],
               "credenciais": {"api": {"tipo": "bearer", "origens": [base.clone()]}}}),
    );
    guardar(&raiz, "api", SEGREDO);
    let a = agente(&raiz, &["http.request", "fs.read"]);
    let f = fluxo(json!({"nome": "x", "passos": [
        {"id": "h", "http": {"url": format!("{base}/eco?q=1"), "credencial": "api"}}]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    let h = passo(&r, "h");
    assert_eq!(h.estado, "falhou", "{h:#?}");
    assert!(h.saida.contains("HTTP 500 em GET"), "{}", h.saida);
    assert!(
        h.saida.contains("/eco") && !h.saida.contains("q=1"),
        "{}",
        h.saida
    );
    assert!(
        h.saida.contains("falhou; recebi authorization"),
        "{}",
        h.saida
    );
    assert!(
        !h.saida.contains(SEGREDO),
        "segredo no relatorio: {}",
        h.saida
    );
    assert!(
        !cru(&a, &r.tarefa).contains(SEGREDO),
        "segredo no task.json"
    );
    assert_eq!(e.toques.load(Ordering::SeqCst), 1);
}

/// A resposta que ECOA o pedido nao devolve o segredo por nenhum caminho de saida: o corpo
/// 2xx, o cabecalho (`resposta: completa`) e o corpo de erro com `aceitar_erro` viram itens
/// tarjados -- direto, na saida da ferramenta ao modelo, no relatorio e no `task.json`.
///
/// RED medido: (1) em `fluxo_http::Execucao::itens`, a tarja retirada (devolvendo os itens
/// crus, `// REPOSTO`): o token aparece nos tres pedidos, na saida e no relatorio. (2) So o
/// `scrub_text` dos segredos conhecidos retirado, ficando o `scrub_secret_like`
/// (`// REPOSTO`): o token sem «Bearer» do `/eco-ok` passa -- a forma sozinha nao basta.
#[tokio::test]
async fn resposta_que_ecoa_o_pedido_nao_vaza_o_segredo() {
    let (base, _) = servidor().await;
    let raiz = tmp("http-eco");
    config(
        &raiz,
        json!({"liberar": [base.clone()],
               "credenciais": {"api": {"tipo": "bearer", "origens": [base.clone()]}}}),
    );
    guardar(&raiz, "api", SEGREDO);
    let pedidos = [
        json!({"url": format!("{base}/eco-ok"), "credencial": "api"}),
        json!({"url": format!("{base}/eco-ok"), "credencial": "api", "resposta": "completa"}),
        json!({"url": format!("{base}/eco"), "credencial": "api", "aceitar_erro": true}),
    ];
    for pedido in &pedidos {
        let itens = http(&raiz, pedido.clone())
            .await
            .unwrap_or_else(|e| panic!("{pedido}: {e}"));
        let t = Value::Array(itens).to_string();
        assert!(!t.contains(SEGREDO), "segredo nos itens de {pedido}: {t}");
        assert!(t.contains("[REDACTED]"), "{pedido}: {t}");
    }
    let itens = http(&raiz, pedidos[1].clone()).await.expect("completa");
    assert_eq!(itens[0]["status"], 200, "{itens:?}");
    assert_eq!(itens[0]["corpo"]["recebi"], "[REDACTED]", "{itens:?}");
    // A saida da ferramenta, que e o que o modelo le.
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: raiz.to_path_buf(),
        timeout: Duration::from_secs(30),
    };
    let ferramenta = HttpTool {
        raiz: raiz.to_path_buf(),
    };
    for pedido in &pedidos {
        let saida = ferramenta
            .run(pedido.clone(), &ctx)
            .await
            .unwrap_or_else(|e| panic!("{pedido}: {e:?}"))
            .content;
        assert!(
            !saida.contains(SEGREDO),
            "segredo na saida da ferramenta: {saida}"
        );
    }
    // O relatorio do fluxo e o `task.json`.
    let a = agente(&raiz, &["http.request", "fs.read"]);
    let f = fluxo(json!({"nome": "x", "passos": [
        {"id": "ok", "http": pedidos[1].clone()},
        {"id": "falha", "http": pedidos[2].clone()}]}));
    let r = fluxos::rodar(&a, &f).await.expect("o fluxo rodou");
    assert!(r.sucesso, "{r:#?}");
    for id in ["ok", "falha"] {
        let h = passo(&r, id);
        assert!(
            !h.saida.contains(SEGREDO),
            "segredo no relatorio: {}",
            h.saida
        );
        assert!(
            !Value::Array(h.itens.clone()).to_string().contains(SEGREDO),
            "segredo nos itens do passo {id}"
        );
    }
    assert!(
        !cru(&a, &r.tarefa).contains(SEGREDO),
        "segredo no task.json"
    );
}

/// A credencial so vai as origens declaradas com ela: pedida para outra origem (mesmo
/// liberada), recusa antes de conectar.
///
/// RED medido: `Credenciada::alcanca` devolvendo `true` (`// REPOSTO`): o Bearer chega ao
/// servidor de outra origem.
#[tokio::test]
async fn credencial_so_vai_as_origens_declaradas() {
    let (dono, _) = servidor().await;
    let (outro, e) = servidor().await;
    let raiz = tmp("http-origem");
    config(
        &raiz,
        json!({"liberar": [dono.clone(), outro.clone()],
               "credenciais": {"api": {"tipo": "bearer", "origens": [dono.clone()]}}}),
    );
    guardar(&raiz, "api", SEGREDO);
    let ok = http(
        &raiz,
        json!({"url": format!("{dono}/protegido"), "credencial": "api"}),
    )
    .await
    .unwrap();
    assert_eq!(ok, vec![json!({"ok": true})]);
    let erro = http(
        &raiz,
        json!({"url": format!("{outro}/quem-vem"), "credencial": "api"}),
    )
    .await
    .unwrap_err();
    assert!(erro.contains("nao vale para"), "{erro}");
    assert_eq!(e.toques.load(Ordering::SeqCst), 0);
    assert!(e.vistos.lock().unwrap().is_empty());
    // Sem segredo guardado, o erro diz o comando.
    config(
        &raiz,
        json!({"liberar": [dono.clone()],
               "credenciais": {"nova": {"tipo": "bearer", "origens": [dono.clone()]}}}),
    );
    let erro = http(
        &raiz,
        json!({"url": format!("{dono}/protegido"), "credencial": "nova"}),
    )
    .await
    .unwrap_err();
    assert!(erro.contains("phxclaw credencial guardar nova"), "{erro}");
}

/// Basico e cabecalho proprio: o segredo do broker vira `Basic base64(usuario:senha)` e o
/// valor do cabecalho declarado.
#[tokio::test]
async fn basico_e_cabecalho_saem_do_broker() {
    let (base, _) = servidor().await;
    let raiz = tmp("http-basico");
    config(
        &raiz,
        json!({"liberar": [base.clone()], "credenciais": {
            "b": {"tipo": "basico", "usuario": "fulano", "origens": [base.clone()]},
            "c": {"tipo": "cabecalho", "cabecalho": "X-Chave", "origens": [base.clone()]}}}),
    );
    guardar(&raiz, "b", SEGREDO);
    guardar(&raiz, "c", SEGREDO);
    let r = http(
        &raiz,
        json!({"url": format!("{base}/basico"), "credencial": "b"}),
    )
    .await
    .unwrap();
    assert_eq!(r[0]["basico"], true);
    let r = http(
        &raiz,
        json!({"url": format!("{base}/basico"), "credencial": "c"}),
    )
    .await
    .unwrap();
    assert_eq!(r[0]["chave"], true);
}

/// OAuth2 nomeado pelo `oauth.rs`: client credentials pede o acesso uma vez e o reaproveita;
/// o 401 invalida e renova uma vez; o refresh token guardado tambem serve. Nenhum segredo
/// em disco fora do broker.
#[tokio::test]
async fn oauth2_client_credentials_e_refresh_pelo_oauth_rs() {
    let (base, e) = servidor().await;
    let raiz = tmp("http-oauth2");
    config(
        &raiz,
        json!({"liberar": [base.clone()], "credenciais": {
            "cc": {"tipo": "oauth2", "origens": [base.clone()], "oauth2": {
                "token": format!("{base}/token"), "cliente_id": "id-1",
                "concessao": "client_credentials", "escopos": ["ler"]}},
            "rt": {"tipo": "oauth2", "origens": [base.clone()], "oauth2": {
                "autorizacao": format!("{base}/authorize"),
                "token": format!("{base}/token"), "cliente_id": "id-2"}}}}),
    );
    guardar(&raiz, "cc", CLIENTE);
    let pedido = json!({"url": format!("{base}/protegido"), "credencial": "cc"});
    assert_eq!(
        http(&raiz, pedido.clone()).await.unwrap(),
        vec![json!({"ok": true})]
    );
    assert_eq!(e.tokens.load(Ordering::SeqCst), 1);
    assert_eq!(
        http(&raiz, pedido.clone()).await.unwrap(),
        vec![json!({"ok": true})]
    );
    assert_eq!(
        e.tokens.load(Ordering::SeqCst),
        1,
        "acesso valido reaproveitado"
    );
    // O servidor passa a recusar o acesso: 401, renova uma vez, e segue.
    *e.acesso.lock().unwrap() = "revogado".into();
    assert_eq!(
        http(&raiz, pedido).await.unwrap(),
        vec![json!({"ok": true})]
    );
    assert_eq!(e.tokens.load(Ordering::SeqCst), 2);

    fluxo_http::guardar_renovacao(&raiz, "rt", SecretValue::new("renova-SEGREDO-1".into()))
        .unwrap();
    let r = http(
        &raiz,
        json!({"url": format!("{base}/protegido"), "credencial": "rt"}),
    )
    .await
    .unwrap();
    assert_eq!(r, vec![json!({"ok": true})]);
    for s in [CLIENTE, "renova-SEGREDO-1"] {
        let mut achados = vec![];
        for e in walk(&raiz) {
            if std::fs::read(&e).is_ok_and(|b| String::from_utf8_lossy(&b).contains(s)) {
                achados.push(e);
            }
        }
        assert!(achados.is_empty(), "{s} em disco: {achados:?}");
    }
}

fn walk(d: &Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![];
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(walk(&p));
        } else {
            v.push(p);
        }
    }
    v
}

// ======================================================================== paginacao e lotes

#[tokio::test]
async fn paginacao_por_cursor_link_e_proximo_com_teto_de_paginas() {
    let (base, _) = servidor().await;
    let raiz = tmp("http-pagina");
    config(&raiz, json!({"liberar": [base.clone()]}));
    let cursor = |max: usize| {
        json!({"url": format!("{base}/cursor"), "query": {"fixo": 1}, "itens": "itens",
               "paginacao": {"cursor": "meta.proximo", "parametro": "cursor", "max_paginas": max}})
    };
    let itens = http(&raiz, cursor(10)).await.unwrap();
    assert_eq!(
        itens,
        vec![
            json!({"n":1}),
            json!({"n":2}),
            json!({"n":3}),
            json!({"n":4})
        ]
    );
    let itens = http(&raiz, cursor(2)).await.unwrap();
    assert_eq!(itens.len(), 3, "o teto de paginas corta: {itens:?}");
    let itens = http(
        &raiz,
        json!({"url": format!("{base}/link"), "paginacao": {"link": true}}),
    )
    .await
    .unwrap();
    assert_eq!(itens, vec![json!({"p":1}), json!({"p":2}), json!({"p":3})]);
    assert!(
        http(
            &raiz,
            json!({"url": format!("{base}/link"), "paginacao": {"max_paginas": 2}})
        )
        .await
        .unwrap_err()
        .contains("exatamente um")
    );
}

#[tokio::test]
async fn lote_manda_n_itens_por_pedido_com_pausa() {
    let (base, e) = servidor().await;
    let raiz = tmp("http-lote");
    config(&raiz, json!({"liberar": [base.clone()]}));
    let itens: Vec<Value> = (1..=5).map(|n| json!({"n": n})).collect();
    let r = http(
        &raiz,
        json!({"metodo": "POST", "url": format!("{base}/lote"),
               "corpo": {"json": {"origem": "teste"}},
               "lote": {"itens": itens, "tamanho": 2, "pausa_ms": 150}}),
    )
    .await
    .unwrap();
    assert_eq!(
        r,
        vec![
            json!({"recebidos": 2}),
            json!({"recebidos": 2}),
            json!({"recebidos": 1})
        ]
    );
    let lotes = e.lotes.lock().unwrap();
    assert_eq!(lotes.len(), 3);
    assert_eq!(lotes[0].1["origem"], "teste");
    assert_eq!(lotes[2].1["itens"], json!([{"n": 5}]));
    for j in lotes.windows(2) {
        assert!(
            j[1].0 - j[0].0 >= Duration::from_millis(140),
            "sem pausa entre os lotes"
        );
    }
}

// ======================================================================== o passo do fluxo

/// O passo `http` do fluxo e a ferramenta `http_request` pelo portao: sem a capacidade, e
/// negado; com ela, a resposta vira itens que o passo seguinte percorre com `por_item`.
#[tokio::test]
async fn passo_http_do_fluxo_vira_itens_pelo_portao() {
    let (base, _) = servidor().await;
    let raiz = tmp("http-passo");
    config(&raiz, json!({"liberar": [base.clone()]}));
    let f = fluxo(
        json!({"nome": "x", "variaveis": {"rota": "itens"}, "passos": [
        {"id": "h", "http": {"url": format!("{base}/{{{{var.rota}}}}"), "itens": "data"}},
        {"id": "cada", "depende": ["h"], "por_item": true, "ferramenta": "eco",
         "args": {"texto": "{{h.id}}"}}]}),
    );
    let negado = fluxos::rodar(&agente(&raiz, &["fs.read"]), &f)
        .await
        .unwrap();
    assert!(
        passo(&negado, "h").saida.contains("http.request"),
        "{negado:#?}"
    );
    let r = fluxos::rodar(&agente(&raiz, &["http.request", "fs.read"]), &f)
        .await
        .unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "h").itens.len(), 2);
    assert_eq!(passo(&r, "cada").itens, vec![json!("1"), json!("2")]);
}

#[test]
fn segredo_e_pedido_invalido_no_passo_http_recusados_na_leitura() {
    for (passo, motivo) in [
        (
            json!({"id": "h", "http": {"url": "https://a/x", "cabecalhos": {"Authorization": "Bearer abcdefghijkl"}}}),
            "credencial",
        ),
        (
            json!({"id": "h", "http": {"url": "https://a/x", "query": {"api_key": "123"}}}),
            "nome de segredo",
        ),
        (
            json!({"id": "h", "http": {"url": "https://a/x"}, "args": {"x": 1}}),
            "sem 'args'",
        ),
        (json!({"id": "h", "http": "{{entrada}}"}), "objeto"),
        (
            json!({"id": "h", "http": {"url": "https://a/x", "metodo": "GET", "corpo": {"json": {}}}}),
            "nao leva corpo",
        ),
        (
            json!({"id": "h", "http": {"url": "https://a/x", "paginacao": {"link": true, "max_paginas": 1000}}}),
            "max_paginas",
        ),
        (
            json!({"id": "h", "http": {"url": "https://a/x", "desconhecido": 1}}),
            "invalido",
        ),
    ] {
        let e =
            fluxos::ler(&json!({"nome": "x", "passos": [passo.clone()]}).to_string()).unwrap_err();
        assert!(e.contains(motivo), "{passo}: {e}");
    }
}

// ======================================================================== gatilho de poll

/// A volta em texto, para o `assert` dizer o que veio em vez do variante esperado.
fn qual(v: &Volta) -> String {
    match v {
        Volta::LinhaDeBase(n) => format!("linha de base com {n}"),
        Volta::Nada => "nada".into(),
        Volta::Disparou(c, n) => format!("disparou {n} (tarefa {})", c.id),
    }
}

fn gatilhos_json(pasta: &Path, v: Value) {
    std::fs::create_dir_all(pasta).unwrap();
    std::fs::write(pasta.join("gatilhos.json"), v.to_string()).unwrap();
}

/// O poll dispara so com os itens NOVOS: a primeira leitura e a linha de base (nada
/// dispara), o item que entrou depois dispara sozinho, e o que ja se viu nao volta nem
/// depois de «reiniciar» (uma `Sondagem` nova sobre o mesmo disco).
///
/// RED medido: em `gatilho_poll::Sondagem::rodar_uma_vez`, o filtro `!vistos.contains(&k)`
/// retirado (`// REPOSTO`): a volta sem item novo dispara de novo com os dois ja vistos.
#[tokio::test]
async fn poll_dispara_so_com_itens_novos_e_lembra_depois_de_reiniciar() {
    let (base, e) = servidor().await;
    let raiz = tmp("poll-novos");
    config(&raiz, json!({"liberar": [base.clone()]}));
    let projeto = raiz.join("projeto");
    let pasta = projeto.join(".phxclaw");
    std::fs::create_dir_all(&pasta).unwrap();
    gravar(
        &projeto,
        "f",
        &json!({"nome": "f", "passos": [
            {"id": "a", "ferramenta": "eco", "args": {"texto": "{{entrada}}"}}]}),
    );
    gatilhos_json(
        &pasta,
        json!({"polls": [{"nome": "p", "http": {"url": format!("{base}/poll")},
                          "chave": "id", "intervalo_s": 60, "fluxo": "f.json"}]}),
    );
    let polls = gatilho_poll::carregar(&pasta).expect("carga dos polls");
    let s = estado_http(&raiz, &["http.request", "fs.read"]);
    *e.poll.lock().unwrap() = vec![json!({"id": 1, "t": "a"}), json!({"id": 2, "t": "b"})];
    let p = Sondagem::new(polls[0].clone(), &raiz);
    // Cada volta com o motivo no panico: a queda de 1 em 113 saiu sem mensagem nenhuma.
    let volta = |n: u8, v: Result<Volta, String>| match v {
        Ok(v) => v,
        Err(e) => panic!("volta {n} do poll falhou: {e}"),
    };
    let v = volta(1, p.rodar_uma_vez(&s).await);
    assert!(matches!(v, Volta::LinhaDeBase(2)), "volta 1: {}", qual(&v));
    assert!(
        s.store.list().expect("lista de tarefas").is_empty(),
        "a primeira leitura disparou"
    );
    let v = volta(2, p.rodar_uma_vez(&s).await);
    assert!(matches!(v, Volta::Nada), "volta 2: {}", qual(&v));
    e.poll.lock().unwrap().push(json!({"id": 3, "t": "c"}));
    let v = volta(3, p.rodar_uma_vez(&s).await);
    let Volta::Disparou(c, n) = v else {
        panic!("o item novo nao disparou: {}", qual(&v));
    };
    assert_eq!(n, 1);
    // O fim da propria execucao, e nao a sondagem do disco: a sondagem aceitava `Failed`
    // e o `relatorio` caia num `unwrap` de `answer` vazio sem dizer por que a tarefa falhou.
    let t = c
        .fim
        .await
        .expect("a execucao do fluxo do poll entrou em panico");
    let Some(resposta) = t.answer.as_deref() else {
        panic!(
            "a tarefa do poll terminou {:?} sem relatorio: erro {:?}",
            t.status, t.error
        );
    };
    let r: fluxos::Relatorio = serde_json::from_str(resposta).unwrap_or_else(|e| {
        panic!(
            "relatorio ilegivel ({e}); estado {:?}, erro {:?}: {resposta}",
            t.status, t.error
        )
    });
    assert_eq!(
        passo(&r, "a").itens,
        vec![json!({"id": 3, "t": "c"})],
        "estado {:?}, erro {:?}",
        t.status,
        t.error
    );
    // «Reinicio»: a sondagem nova le o estado do disco e nao redispara nada.
    let p2 = Sondagem::new(polls[0].clone(), &raiz);
    let v = volta(4, p2.rodar_uma_vez(&s).await);
    assert!(matches!(v, Volta::Nada), "volta 4: {}", qual(&v));
}

/// O poll passa pelo portao unico: sem `http.request` concedida ao agente do servidor,
/// nenhuma requisicao sai -- nem a com a credencial do operador --, a volta falha dizendo a
/// capacidade, e a recusa fica na evidencia do poll. Concedida, o mesmo poll sai.
///
/// RED medido: em `gatilho_poll::Sondagem::rodar_uma_vez`, o `self.pedir(s)` trocado pelo
/// `fluxo_http::executar` direto (`// REPOSTO`): o Bearer chega ao servidor falso sem a
/// capacidade concedida.
#[tokio::test]
async fn poll_sem_a_capacidade_nao_faz_rede_nem_com_credencial() {
    let (base, e) = servidor().await;
    let raiz = tmp("poll-capacidade");
    config(
        &raiz,
        json!({"liberar": [base.clone()],
               "credenciais": {"api": {"tipo": "bearer", "origens": [base.clone()]}}}),
    );
    guardar(&raiz, "api", SEGREDO);
    let pasta = raiz.join("projeto").join(".phxclaw");
    gatilhos_json(
        &pasta,
        json!({"polls": [{"nome": "p",
                          "http": {"url": format!("{base}/quem-vem"), "credencial": "api"},
                          "intervalo_s": 60, "disparar_primeira": true,
                          "objetivo": "resuma {itens}"}]}),
    );
    let polls = gatilho_poll::carregar(&pasta).expect("carga dos polls");
    let p = Sondagem::new(polls[0].clone(), &raiz);
    let negado = estado_http(&raiz, &["fs.read"]);
    let erro = match p.rodar_uma_vez(&negado).await {
        Err(e) => e,
        Ok(_) => panic!("o poll rodou sem a capacidade http.request"),
    };
    assert!(erro.contains("http.request"), "{erro}");
    assert_eq!(
        e.toques.load(Ordering::SeqCst),
        0,
        "o pedido do poll chegou ao servidor sem a capacidade"
    );
    assert!(e.vistos.lock().unwrap().is_empty());
    let ev = std::fs::read_to_string(raiz.join("gatilhos").join("poll-p.evidence.jsonl"))
        .expect("a evidencia do poll");
    assert!(
        ev.contains("http_request") && ev.contains("http.request"),
        "{ev}"
    );
    assert!(!ev.contains(SEGREDO), "segredo na evidencia: {ev}");

    let concedido = estado_http(&raiz, &["http.request", "fs.read"]);
    let v = p
        .rodar_uma_vez(&concedido)
        .await
        .unwrap_or_else(|e| panic!("poll com a capacidade: {e}"));
    let Volta::Disparou(c, n) = v else {
        panic!("disparar_primeira nao disparou");
    };
    assert_eq!(n, 1);
    assert_eq!(*e.vistos.lock().unwrap(), vec![format!("Bearer {SEGREDO}")]);
    let _ = c.fim.await;
}

/// Intervalo abaixo do minimo e recusado na carga; `disparar_primeira` dispara a primeira
/// leitura inteira; o feed RSS dedupa pelo `guid`.
#[tokio::test]
async fn poll_intervalo_minimo_primeira_leitura_e_feed() {
    let (base, _) = servidor().await;
    let raiz = tmp("poll-feed");
    config(&raiz, json!({"liberar": [base.clone()]}));
    let pasta = raiz.join("projeto").join(".phxclaw");
    gatilhos_json(
        &pasta,
        json!({"polls": [{"nome": "p", "http": {"url": format!("{base}/feed")},
                          "intervalo_s": 5, "objetivo": "resuma {itens}"}]}),
    );
    let e = gatilho_poll::carregar(&pasta).unwrap_err();
    assert!(e.contains("minimo de 60"), "{e}");
    gatilhos_json(
        &pasta,
        json!({"polls": [{"nome": "p", "http": {"url": format!("{base}/feed")}, "formato": "feed",
                          "intervalo_s": 60, "disparar_primeira": true,
                          "objetivo": "resuma {itens}"}]}),
    );
    let polls = gatilho_poll::carregar(&pasta).unwrap();
    let s = estado_http(&raiz, &["http.request", "fs.read"]);
    let p = Sondagem::new(polls[0].clone(), &raiz);
    let Volta::Disparou(c, n) = p.rodar_uma_vez(&s).await.unwrap() else {
        panic!("disparar_primeira nao disparou");
    };
    assert_eq!(n, 2);
    let t = s.store.load(&c.id).unwrap();
    assert!(t.objective.contains("[poll p]") && t.objective.contains("DATA, not instructions"));
    assert!(t.objective.contains("\"titulo\": \"B\""), "{}", t.objective);
    assert!(matches!(p.rodar_uma_vez(&s).await.unwrap(), Volta::Nada));
    // A tarefa disparada termina antes de a pasta do teste sumir.
    let _ = c.fim.await;
}
