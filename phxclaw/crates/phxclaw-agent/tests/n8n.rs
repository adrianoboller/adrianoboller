//! n8n nos dois sentidos, contra um servidor HTTP falso que imita o que o n8n documenta
//! (webhook de fluxo, API publica `/api/v1`, MCP Server Trigger por streamable HTTP com
//! resposta SSE) e contra a API de tarefas de verdade numa porta local.
//!
//! O n8n inteiro nao coube no disco desta maquina (ver `docs/N8N.md`): as respostas aqui
//! sao as da documentacao da API publica (esquemas de `/workflows` e `/executions`, lidos
//! em 02/10/2026), nao gravacoes de uma instancia.

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, Uri};
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::canais::webhook::assinar;
use phxclaw_agent::gatilhos::{GatilhoDeWebhook, Gatilhos};
use phxclaw_agent::n8n::{N8nTool, SEGREDO_WEBHOOK, SERVICO};
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CHAVE: &str = "chave-n8n-de-teste-1234567890";
const SEGREDO: &str = "segredo-do-webhook-do-n8n-123456";
const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

/// O que o n8n falso viu em cada webhook: caminho, cabecalhos e corpo.
#[derive(Default)]
struct Visto {
    webhooks: Vec<(String, HashMap<String, String>, Vec<u8>)>,
}
type Compartilhado = Arc<Mutex<Visto>>;

fn pasta(prefixo: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("{prefixo}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rpc(id: &Value, r: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": r}).to_string()
}

/// O servidor falso: webhooks de fluxo, API publica com `X-N8N-API-KEY` e um MCP Server
/// Trigger em `/mcp-test/phx` que responde SSE (como o SDK do n8n faz por padrao).
async fn n8n_falso() -> (String, Compartilhado) {
    let e: Compartilhado = Arc::default();
    let e2 = e.clone();
    let app = axum::Router::new().fallback(move |u: Uri, cab: HeaderMap, corpo: Bytes| {
        let e = e2.clone();
        async move {
            let caminho = u.path().to_string();
            let chave = cab
                .get("x-n8n-api-key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let consulta: HashMap<String, String> =
                url::form_urlencoded::parse(u.query().unwrap_or("").as_bytes())
                    .into_owned()
                    .collect();
            let (status, tipo, corpo): (StatusCode, &str, String) = if caminho
                .starts_with("/webhook/")
                || caminho.starts_with("/webhook-test/")
            {
                let h: HashMap<String, String> = cab
                    .iter()
                    .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                e.lock().unwrap().webhooks.push((caminho.clone(), h, corpo.to_vec()));
                // O texto do no Webhook com "Respond: Immediately".
                (
                    StatusCode::OK,
                    "application/json",
                    json!({"message": "Workflow was started"}).to_string(),
                )
            } else if caminho.starts_with("/api/v1/") && chave != CHAVE {
                (
                    StatusCode::UNAUTHORIZED,
                    "application/json",
                    json!({"message": "'X-N8N-API-KEY' header required"}).to_string(),
                )
            } else if caminho == "/api/v1/workflows" && consulta.get("name").map(String::as_str) == Some("eco") {
                // Servidor que ecoa a chave no erro: ela nao pode voltar ao modelo.
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "application/json",
                    json!({"message": format!("falhou com {chave}")}).to_string(),
                )
            } else if caminho == "/api/v1/workflows" {
                assert_eq!(consulta.get("limit").map(String::as_str), Some("20"));
                (
                    StatusCode::OK,
                    "application/json",
                    json!({"data": [
                        {"id": "2tUt1wbLX592XDdX", "name": "Pedido novo", "active": true,
                         "createdAt": "2026-10-01T10:00:00.000Z", "updatedAt": "2026-10-02T08:30:00.000Z",
                         "isArchived": false, "versionId": "v1", "triggerCount": 1, "nodes": [], "connections": {}},
                        {"id": "9kLm2nOpQ3rS4tU5", "name": "Relatorio diario", "active": false,
                         "createdAt": "2026-09-20T10:00:00.000Z", "updatedAt": "2026-09-21T08:30:00.000Z",
                         "isArchived": false, "versionId": "v3", "triggerCount": 0, "nodes": [], "connections": {}}
                    ], "nextCursor": null})
                    .to_string(),
                )
            } else if caminho == "/api/v1/executions" {
                assert_eq!(consulta.get("workflowId").map(String::as_str), Some("2tUt1wbLX592XDdX"));
                (
                    StatusCode::OK,
                    "application/json",
                    json!({"data": [
                        {"id": "1010", "finished": true, "mode": "webhook", "retryOf": null,
                         "retrySuccessId": null, "status": "success",
                         "startedAt": "2026-10-02T09:00:00.000Z", "stoppedAt": "2026-10-02T09:00:02.000Z",
                         "workflowId": "2tUt1wbLX592XDdX", "waitTill": null, "storedAt": "db", "jsonSizeBytes": 812},
                        {"id": "1009", "finished": false, "mode": "webhook", "retryOf": null,
                         "retrySuccessId": null, "status": "error",
                         "startedAt": "2026-10-02T08:00:00.000Z", "stoppedAt": "2026-10-02T08:00:01.000Z",
                         "workflowId": "2tUt1wbLX592XDdX", "waitTill": null, "storedAt": "db", "jsonSizeBytes": 320}
                    ], "nextCursor": null})
                    .to_string(),
                )
            } else if caminho == "/api/v1/executions/1010" {
                (
                    StatusCode::OK,
                    "application/json",
                    json!({"id": "1010", "finished": true, "mode": "webhook", "status": "success",
                           "startedAt": "2026-10-02T09:00:00.000Z", "stoppedAt": "2026-10-02T09:00:02.000Z",
                           "workflowId": "2tUt1wbLX592XDdX", "waitTill": null})
                    .to_string(),
                )
            } else if caminho == "/mcp-test/phx" {
                let v: Value = serde_json::from_slice(&corpo).unwrap();
                let id = v.get("id").cloned().unwrap_or(Value::Null);
                let r = match v["method"].as_str().unwrap_or("") {
                    "initialize" => rpc(
                        &id,
                        json!({"protocolVersion": "2025-03-26", "capabilities": {"tools": {}},
                               "serverInfo": {"name": "n8n-mcp-server", "version": "1"}}),
                    ),
                    "notifications/initialized" => {
                        return axum::response::Response::builder()
                            .status(StatusCode::ACCEPTED)
                            .body(axum::body::Body::empty())
                            .unwrap();
                    }
                    "tools/list" => rpc(
                        &id,
                        json!({"tools": [{"name": "Buscar_pedido", "description": "Busca um pedido pelo numero",
                            "inputSchema": {"type": "object", "properties": {"numero": {"type": "string"}}}}]}),
                    ),
                    "tools/call" => rpc(
                        &id,
                        json!({"content": [{"type": "text", "text": format!(
                            "pedido {} entregue", v.pointer("/params/arguments/numero").and_then(Value::as_str).unwrap_or("?"))}]}),
                    ),
                    _ => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "nao sei"}}).to_string(),
                };
                (
                    StatusCode::OK,
                    "text/event-stream",
                    format!("event: message\ndata: {r}\n\n"),
                )
            } else {
                (StatusCode::NOT_FOUND, "application/json", String::new())
            };
            axum::response::Response::builder()
                .status(status)
                .header("content-type", tipo)
                .body(axum::body::Body::from(corpo))
                .unwrap()
        }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, e)
}

fn ctx() -> ToolContext {
    ToolContext {
        task_id: "t1".into(),
        workdir: pasta("phx-n8n-work"),
        timeout: Duration::from_secs(10),
    }
}

async fn roda(t: &N8nTool, args: Value) -> Result<Value, ToolError> {
    let saida = t.run(args, &ctx()).await?;
    Ok(serde_json::from_str(&saida.content).unwrap_or_else(|_| json!(saida.content)))
}

// ------------------------------------------------------------ PhxClaw -> n8n (ferramenta)

/// `run` manda o corpo JSON ao webhook do fluxo com a MESMA assinatura do canal de webhook
/// (`sha256=HMAC(segredo, carimbo.corpo)`), ou o segredo em claro para o Header Auth do
/// n8n, ou nada; e `test=true` vai para a URL de teste.
#[tokio::test]
async fn run_assina_o_corpo_com_o_mesmo_hmac_do_canal_de_webhook() {
    let (base, visto) = n8n_falso().await;
    let raiz = pasta("phx-n8n-raiz");
    SEGREDO_WEBHOOK.guardar(&raiz, SEGREDO).unwrap();
    let t = N8nTool::novo(&base, &raiz).unwrap();

    let r = roda(
        &t,
        json!({"action": "run", "path": "pedido", "body": {"numero": 42, "cliente": "Ana"}}),
    )
    .await
    .unwrap();
    assert_eq!(r["status"], 200, "{r}");
    assert_eq!(r["response"]["message"], "Workflow was started", "{r}");
    {
        let v = visto.lock().unwrap();
        let (caminho, h, corpo) = &v.webhooks[0];
        assert_eq!(caminho, "/webhook/pedido");
        assert_eq!(h["content-type"], "application/json");
        let corpo_json: Value = serde_json::from_slice(corpo).unwrap();
        assert_eq!(corpo_json, json!({"numero": 42, "cliente": "Ana"}));
        // Conferido como o no de codigo do n8n confere: recalculando com o segredo.
        let carimbo: i64 = h["x-phxclaw-carimbo"].parse().unwrap();
        assert!((chrono::Utc::now().timestamp() - carimbo).abs() < 60);
        assert_eq!(h["x-phxclaw-assinatura"], assinar(SEGREDO, carimbo, corpo));
        assert!(!h.contains_key("x-phxclaw-segredo"), "{h:?}");
    }

    roda(&t, json!({"action": "run", "path": "/webhook/pedido", "test": true, "auth": "header", "body": {}}))
        .await
        .unwrap();
    roda(
        &t,
        json!({"action": "run", "path": "pedido", "auth": "none"}),
    )
    .await
    .unwrap();
    let v = visto.lock().unwrap();
    assert_eq!(v.webhooks[1].0, "/webhook-test/pedido");
    assert_eq!(v.webhooks[1].1["x-phxclaw-segredo"], SEGREDO);
    assert!(!v.webhooks[1].1.contains_key("x-phxclaw-assinatura"));
    assert_eq!(v.webhooks[2].0, "/webhook/pedido");
    assert!(!v.webhooks[2].1.contains_key("x-phxclaw-segredo"));
    assert!(!v.webhooks[2].1.contains_key("x-phxclaw-assinatura"));
    assert_eq!(v.webhooks[2].2, b"{}");
}

/// Sem segredo guardado o `run` sai sem assinatura (o webhook e porta publica do fluxo),
/// e o corpo que nao e objeto, o caminho fora de `/webhook/` e o teto se recusam ANTES de
/// qualquer pedido.
#[tokio::test]
async fn run_sem_segredo_nao_assina_e_recusa_argumento_errado_antes_de_pedir() {
    let (base, visto) = n8n_falso().await;
    let raiz = pasta("phx-n8n-raiz");
    let t = N8nTool::novo(&base, &raiz).unwrap();
    roda(
        &t,
        json!({"action": "run", "path": "pedido", "body": {"a": 1}}),
    )
    .await
    .unwrap();
    assert!(
        !visto.lock().unwrap().webhooks[0]
            .1
            .contains_key("x-phxclaw-assinatura")
    );

    for args in [
        json!({"action": "run", "path": "pedido", "body": [1, 2]}),
        json!({"action": "run", "path": "../api/v1/workflows"}),
        json!({"action": "run", "path": ""}),
        json!({"action": "run", "path": "pedido", "auth": "basic"}),
        json!({"action": "run", "path": "pedido", "body": {"x": "a".repeat(70_000)}}),
        json!({"action": "apagar"}),
        json!({}),
    ] {
        assert!(roda(&t, args.clone()).await.is_err(), "{args}");
    }
    assert_eq!(
        visto.lock().unwrap().webhooks.len(),
        1,
        "pedido saiu com argumento errado"
    );
}

/// `list` e `status` leem a API publica com a chave do broker. Sem chave, o erro cita o
/// comando que a guarda; e a chave nunca volta ao modelo, nem quando o n8n a ecoa.
#[tokio::test]
async fn list_e_status_pedem_a_chave_do_broker_e_nunca_a_devolvem() {
    let (base, _) = n8n_falso().await;
    let raiz = pasta("phx-n8n-raiz");
    let t = N8nTool::novo(&base, &raiz).unwrap();
    let e = roda(&t, json!({"action": "list"}))
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("phxclaw n8n chave"), "{e}");

    SERVICO.guardar(&raiz, CHAVE).unwrap();
    let r = roda(&t, json!({"action": "list", "active": true}))
        .await
        .unwrap();
    let nomes: Vec<&str> = r["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(nomes, ["Pedido novo", "Relatorio diario"], "{r}");
    assert_eq!(r["workflows"][0]["id"], "2tUt1wbLX592XDdX");
    assert!(
        r["workflows"][0].get("nodes").is_none(),
        "o resumo nao carrega os nos"
    );

    let r = roda(
        &t,
        json!({"action": "status", "workflow_id": "2tUt1wbLX592XDdX"}),
    )
    .await
    .unwrap();
    assert_eq!(r["executions"][0]["status"], "success", "{r}");
    assert_eq!(r["executions"][1]["status"], "error", "{r}");
    let r = roda(&t, json!({"action": "status", "execution_id": "1010"}))
        .await
        .unwrap();
    assert_eq!(
        r["execution"]["stoppedAt"], "2026-10-02T09:00:02.000Z",
        "{r}"
    );

    let e = roda(&t, json!({"action": "list", "name": "eco"}))
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("HTTP 500"), "{e}");
    assert!(!e.contains(CHAVE), "a chave voltou no erro: {e}");
}

/// A politica de destino: HTTP sem TLS so em loopback, e a ferramenta nunca sai da origem
/// configurada (o `path` nao abre outra rota).
#[test]
fn n8n_fora_de_loopback_exige_tls() {
    let raiz = pasta("phx-n8n-raiz");
    let e = N8nTool::novo("http://n8n.exemplo.com", &raiz)
        .err()
        .unwrap();
    assert!(e.contains("HTTP sem TLS so em loopback"), "{e}");
    assert!(N8nTool::novo("http://127.0.0.1:5678", &raiz).is_ok());
    assert!(N8nTool::novo("https://n8n.exemplo.com/", &raiz).is_ok());
}

/// Pelo portao do motor: sem `automacao.n8n` concedida a ferramenta e negada e nenhum
/// pedido sai; com ela, o modelo dispara o fluxo e recebe a resposta do n8n.
#[tokio::test]
async fn portao_do_motor_nega_sem_automacao_n8n() {
    let (base, visto) = n8n_falso().await;
    let raiz = pasta("phx-n8n-raiz");
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let roteiro = || {
        vec![
            ScriptedLlm::call(
                "c1",
                "n8n_workflow",
                json!({"action": "run", "path": "pedido", "body": {"n": 1}}),
            ),
            ScriptedLlm::text("feito"),
        ]
    };
    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(N8nTool::novo(&base, &raiz).unwrap())];
    let negado = Agent::new(
        Arc::new(ScriptedLlm::new(roteiro())),
        tools.clone(),
        AgentConfig::default().grant(&["fs.read"]),
        store.clone(),
    );
    let fim = negado
        .run(
            Task::new("dispare o pedido", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(fim.status, TaskStatus::Completed, "{:?}", fim.error);
    assert!(
        visto.lock().unwrap().webhooks.is_empty(),
        "pedido saiu negado"
    );

    let llm = Arc::new(ScriptedLlm::new(roteiro()));
    let concedido = Agent::new(
        llm.clone(),
        tools,
        AgentConfig::default().grant(&["automacao.n8n"]),
        store,
    );
    let fim = concedido
        .run(
            Task::new("dispare o pedido", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(fim.status, TaskStatus::Completed, "{:?}", fim.error);
    assert_eq!(visto.lock().unwrap().webhooks.len(), 1);
    let vistos = llm.seen.lock().unwrap();
    let r = &vistos[1].0.last().unwrap().content;
    assert!(r.contains("Workflow was started"), "{r}");
}

// ------------------------------------------------------------ n8n -> PhxClaw

async fn subir(gatilhos: Option<Arc<Gatilhos>>) -> (String, ApiState) {
    let dir = pasta("phx-n8n-api");
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(
            "resposta do agente",
        )]));
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool), Arc::new(ReadFileTool)];
        Ok(Agent::new(
            llm,
            tools,
            AgentConfig::default().grant(&["fs.write", "fs.read"]),
            st2.clone(),
        ))
    });
    let state = ApiState {
        store,
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let mut app = router(state.clone());
    if let Some(g) = gatilhos {
        app = app.merge(phxclaw_agent::gatilhos::router(state.clone(), g));
    }
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, state)
}

/// (c) O webhook do n8n (no HTTP Request) dispara um gatilho: pelo segredo em claro
/// (Header Auth) ou pela MESMA assinatura HMAC do `run`, com carimbo -- e assinatura
/// errada ou carimbo velho e 401, sem criar tarefa.
#[tokio::test]
async fn webhook_do_n8n_dispara_o_gatilho_por_segredo_ou_por_hmac() {
    let g = Arc::new(Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "n8n-resultado".into(),
            objetivo: "Trate o resultado do fluxo: {corpo}".into(),
            segredo: Some(SEGREDO.into()),
        }],
    });
    let (base, state) = subir(Some(g)).await;
    let cli = reqwest::Client::new();
    let url = format!("{base}/v1/triggers/n8n-resultado");
    let corpo = json!({"executionId": "1010", "status": "success"}).to_string();

    let r = cli
        .post(&url)
        .header("x-phxclaw-segredo", SEGREDO)
        .body(corpo.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);

    let carimbo = chrono::Utc::now().timestamp();
    let r = cli
        .post(&url)
        .header("x-phxclaw-carimbo", carimbo.to_string())
        .header(
            "x-phxclaw-assinatura",
            assinar(SEGREDO, carimbo, corpo.as_bytes()),
        )
        .body(corpo.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202, "{}", r.text().await.unwrap());

    // Assinatura de outro corpo, segredo errado e carimbo de 10 minutos atras: 401.
    for (c, a) in [
        (carimbo, assinar(SEGREDO, carimbo, b"outro corpo")),
        (
            carimbo,
            assinar("segredo-errado-1234567890123456", carimbo, corpo.as_bytes()),
        ),
        (
            carimbo - 600,
            assinar(SEGREDO, carimbo - 600, corpo.as_bytes()),
        ),
    ] {
        let r = cli
            .post(&url)
            .header("x-phxclaw-carimbo", c.to_string())
            .header("x-phxclaw-assinatura", a)
            .body(corpo.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
    }
    let tarefas = state.store.list().unwrap();
    assert_eq!(tarefas.len(), 2, "{tarefas:?}");
    assert!(
        tarefas
            .iter()
            .all(|t| t.objective.contains("\"executionId\":\"1010\""))
    );
}

/// (a) O MCP Client Tool do n8n fala com `phxclaw servir` por streamable HTTP em `/mcp`:
/// o MESMO `responder` do `mcp-serve`, com o Bearer da API. Notificacao e 202 sem corpo;
/// GET (fluxo iniciado pelo servidor) e 405.
#[tokio::test]
async fn mcp_client_tool_do_n8n_fala_com_o_servir_por_streamable_http() {
    let (base, state) = subir(None).await;
    let cli = reqwest::Client::new();
    let url = format!("{base}/mcp");
    let pede = |m: Value, sessao: Option<String>| {
        let cli = cli.clone();
        let url = url.clone();
        async move {
            let mut p = cli
                .post(&url)
                .bearer_auth(TOKEN)
                .header("accept", "application/json, text/event-stream");
            if let Some(s) = sessao {
                p = p.header("mcp-session-id", s);
            }
            p.json(&m).send().await.unwrap()
        }
    };
    let r = cli
        .post(&url)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "sem Bearer");

    let r = pede(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "n8n", "version": "2"}}}), None)
        .await;
    assert_eq!(r.status(), 200);
    // O servidor nomeia a sessao (um UUID); o cliente a repete, como o SDK do n8n faz.
    let sessao = r.headers()["mcp-session-id"].to_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&sessao).is_ok(), "{sessao}");
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["result"]["serverInfo"]["name"], "phxclaw", "{v}");

    let r = pede(
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        Some(sessao.clone()),
    )
    .await;
    assert_eq!(r.status(), 202);

    let r = pede(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        Some(sessao.clone()),
    )
    .await;
    assert_eq!(r.headers()["mcp-session-id"].to_str().unwrap(), sessao);
    let v: Value = r.json().await.unwrap();
    let nomes: Vec<&str> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        nomes.contains(&"write_file") && nomes.contains(&"read_file"),
        "{nomes:?}"
    );

    let r = pede(
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "write_file", "arguments": {"path": "nota.txt", "content": "do n8n"}}}),
        Some(sessao.clone()),
    )
    .await;
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["result"]["isError"], false, "{v}");
    let r = pede(
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "read_file", "arguments": {"path": "nota.txt"}}}),
        Some(sessao.clone()),
    )
    .await;
    let v: Value = r.json().await.unwrap();
    assert!(
        v["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("do n8n"),
        "{v}"
    );
    // A sessao e uma pasta de trabalho do servidor (`tasks/<sessao>/work`), e um id que
    // nao tem forma de UUID nao vira caminho: ganha sessao nova.
    assert!(state.store.workdir(&sessao).join("nota.txt").is_file());
    let r = pede(
        json!({"jsonrpc": "2.0", "id": 5, "method": "ping"}),
        Some("../../etc".into()),
    )
    .await;
    let outra = r.headers()["mcp-session-id"].to_str().unwrap().to_string();
    assert!(
        uuid::Uuid::parse_str(&outra).is_ok() && outra != sessao,
        "{outra}"
    );

    let r = cli.get(&url).bearer_auth(TOKEN).send().await.unwrap();
    assert_eq!(r.status(), 405);
    let r = pede(
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call",
        "params": {"name": "shell", "arguments": {"command": "id"}}}),
        Some(sessao),
    )
    .await;
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["result"]["isError"], true, "{v}");
}

/// (b) O MCP Server Trigger do n8n vira ferramenta do agente pela configuracao de MCP que
/// ja existe (`url`): a resposta SSE do SDK do n8n e lida pelo cliente do runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_server_trigger_do_n8n_vira_ferramenta_do_agente() {
    let (base, _) = n8n_falso().await;
    let cfg: phxclaw_agent::mcp::ConfigMcp = serde_json::from_value(json!({
        "servidores": [{"nome": "n8n", "url": format!("{base}/mcp-test/phx")}]
    }))
    .unwrap();
    // `carregar_config` e sincrono e sobe um runtime proprio: fora do runtime do teste.
    let (tools, avisos) = tokio::task::spawn_blocking(move || {
        phxclaw_agent::mcp::carregar_config(&cfg, &pasta("phx-n8n-mcp"))
    })
    .await
    .unwrap();
    assert!(avisos.is_empty(), "{avisos:?}");
    let nomes: Vec<String> = tools.iter().map(|t| t.spec().name).collect();
    assert_eq!(nomes, ["mcp__n8n__Buscar_pedido"]);
    assert_eq!(tools[0].capability(), "mcp.n8n");
    let saida = tools[0].run(json!({"numero": "42"}), &ctx()).await.unwrap();
    assert!(
        saida.content.contains("pedido 42 entregue"),
        "{}",
        saida.content
    );
    for t in &tools {
        t.finish("t1").await;
    }
}
