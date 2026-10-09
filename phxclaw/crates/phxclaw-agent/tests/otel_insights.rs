//! A exportacao OpenTelemetry contra um COLETOR FALSO local (que confere o JSON do OTLP:
//! `resourceSpans/scopeSpans/spans`, ids hex de 16/8 bytes, nomes de atributo do conjunto
//! fechado, nenhum dado sensivel) e o painel `GET /v1/insights` pelo fio HTTP, atras do RBAC.

use axum::extract::{Path as Caminho, State};
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::metricas::{FimDeTarefa, Metricas};
use phxclaw_agent::otel::{ATRIBUTOS, Config, Exportador};
use phxclaw_agent::rbac::{self, Usuarios};
use phxclaw_agent::*;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";
/// O que NUNCA pode sair num atributo: argumento de ferramenta, objetivo, nome de credencial.
const SENSIVEL: &str = "argumento-sensivel-7f3a";
const CREDENCIAL: &str = "credencial-do-cliente-acme";
const OBJETIVO: &str = "objetivo-privado-do-usuario";

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-otel-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Eco;
impl Tool for Eco {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "eco".into(),
            description: "eco".into(),
            parameters: json!({"type": "object"}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _c: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move { Ok(ToolOutput::text(args.to_string())) })
    }
}

/// O agente de cada execucao: o modelo de roteiro chama `eco` com o argumento sensivel e
/// depois responde.
fn estado(raiz: &Path) -> ApiState {
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let st = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![
                ScriptedLlm::call(
                    "c1",
                    "eco",
                    json!({"texto": SENSIVEL, "credencial": CREDENCIAL}),
                ),
                ScriptedLlm::text("pronto"),
                ScriptedLlm::text("pronto"),
            ])),
            vec![Arc::new(Eco) as Arc<dyn Tool>],
            AgentConfig::default().grant(&["fs.read"]),
            st.clone(),
        ))
    });
    ApiState {
        usuarios: Usuarios::da_pasta(raiz),
        store,
        factory,
        default_model: "roteiro".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(10_000)),
    }
}

type Recebidos = Arc<Mutex<Vec<(String, String, Value)>>>;

/// O coletor falso: guarda (caminho, content-type, corpo) de cada POST e responde 200.
async fn coletor() -> (String, Recebidos) {
    let r: Recebidos = Arc::new(Mutex::new(vec![]));
    async fn receber(
        State(r): State<Recebidos>,
        Caminho(c): Caminho<String>,
        h: axum::http::HeaderMap,
        corpo: String,
    ) -> &'static str {
        let ct = h
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        r.lock()
            .unwrap()
            .push((c, ct, serde_json::from_str(&corpo).unwrap_or(Value::Null)));
        "{}"
    }
    let app = axum::Router::new()
        .route("/v1/{c}", axum::routing::post(receber))
        .with_state(r.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, r)
}

fn exportador(base: &str) -> &'static Exportador {
    let e: &'static Exportador = Box::leak(Box::new(Exportador::novo()));
    e.definir(Some(Config::nova(base, "phxclaw-teste", "teste").unwrap()));
    e
}

fn eh_hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn chaves(attrs: &Value) -> Vec<String> {
    attrs
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|a| a["key"].as_str().unwrap().to_string())
        .collect()
}

fn attr<'a>(span: &'a Value, k: &str) -> Option<&'a Value> {
    span["attributes"]
        .as_array()?
        .iter()
        .find(|a| a["key"] == k)
        .map(|a| &a["value"])
}

/// Confere a forma do `ExportTraceServiceRequest` e devolve os spans.
fn conferir_traces(corpo: &Value, trace: &str) -> Vec<Value> {
    let rs = &corpo["resourceSpans"][0];
    let servico = rs["resource"]["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["key"] == "service.name")
        .unwrap();
    assert_eq!(servico["value"]["stringValue"], "phxclaw-teste");
    let ss = &rs["scopeSpans"][0];
    assert_eq!(ss["scope"]["name"], "phxclaw");
    let spans = ss["spans"].as_array().unwrap().clone();
    let ids: BTreeSet<String> = spans
        .iter()
        .map(|s| s["spanId"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids.len(), spans.len(), "spanId repetido");
    let mut raizes = 0;
    for s in &spans {
        assert!(eh_hex(s["traceId"].as_str().unwrap(), 32), "traceId: {s}");
        assert_eq!(s["traceId"], trace);
        assert!(eh_hex(s["spanId"].as_str().unwrap(), 16), "spanId: {s}");
        match s.get("parentSpanId").and_then(Value::as_str) {
            Some(p) => {
                assert!(eh_hex(p, 16), "parentSpanId: {s}");
                assert!(ids.contains(p), "pai fora do trace: {s}");
            }
            None => raizes += 1,
        }
        let ini: u128 = s["startTimeUnixNano"].as_str().unwrap().parse().unwrap();
        let fim: u128 = s["endTimeUnixNano"].as_str().unwrap().parse().unwrap();
        assert!(fim >= ini && ini > 1_600_000_000_000_000_000, "{s}");
        assert_eq!(s["kind"], 1);
        assert!(
            [
                "phxclaw.tarefa",
                "phxclaw.fluxo",
                "phxclaw.modelo",
                "phxclaw.ferramenta"
            ]
            .contains(&s["name"].as_str().unwrap()),
            "{s}"
        );
        for k in chaves(&s["attributes"]) {
            assert!(
                ATRIBUTOS.contains(&k.as_str()),
                "atributo fora da lista: {k}"
            );
        }
        for e in s["events"].as_array().unwrap_or(&vec![]) {
            for k in chaves(&e["attributes"]) {
                assert!(
                    ATRIBUTOS.contains(&k.as_str()),
                    "atributo de evento fora da lista: {k}"
                );
            }
        }
    }
    assert_eq!(raizes, 1, "um trace tem UMA raiz");
    let texto = corpo.to_string();
    for proibido in [SENSIVEL, CREDENCIAL, OBJETIVO] {
        assert!(
            !texto.contains(proibido),
            "dado sensivel no trace: {proibido}"
        );
    }
    spans
}

fn uuid_hex(id: &str) -> String {
    id.replace('-', "")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn desligado_nao_monta_nem_le_o_disco() {
    let raiz = tmp("desligado");
    let s = estado(&raiz);
    let e: &'static Exportador = Box::leak(Box::new(Exportador::novo()));
    let mut t = Task::new(OBJETIVO, "roteiro");
    t.status = TaskStatus::Completed;
    s.store.save(&t).unwrap();
    assert!(e.exportar_tarefa(&s.store, &t.id).is_none());
    assert!(
        e.exportar_metricas(&{
            let m = Metricas::nova();
            m.definir(true);
            m
        })
        .is_none()
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        e.montados.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "trabalho feito antes do interruptor"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn traces_da_tarefa_no_otlp_json_com_os_ids_da_evidencia() {
    let (base, recebidos) = coletor().await;
    let raiz = tmp("tarefa");
    let s = estado(&raiz);
    let n: phxclaw_agent::api::NovaTarefa =
        serde_json::from_value(json!({"objective": OBJETIVO})).unwrap();
    let c = phxclaw_agent::api::criar_tarefa(&s, n).unwrap();
    let t = c.fim.await.unwrap();
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    let e = exportador(&base);
    let st = e.exportar_tarefa(&s.store, &t.id).unwrap().await.unwrap();
    assert_eq!(st, Ok(200));
    let r = recebidos.lock().unwrap().clone();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, "traces");
    assert_eq!(r[0].1, "application/json");
    let spans = conferir_traces(&r[0].2, &uuid_hex(&t.id));
    let raiz_span = spans
        .iter()
        .find(|s| s.get("parentSpanId").is_none())
        .unwrap();
    assert_eq!(raiz_span["name"], "phxclaw.tarefa");
    assert_eq!(
        attr(raiz_span, "phxclaw.tarefa.id").unwrap()["stringValue"],
        t.id.as_str()
    );
    assert_eq!(
        attr(raiz_span, "phxclaw.tarefa.estado").unwrap()["stringValue"],
        "completed"
    );
    // A chamada de ferramenta leva o id do registro de evidencia que a gravou.
    let ev: Vec<Value> = std::fs::read_to_string(s.store.evidence_path(&t.id))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let id_ev = ev.iter().find(|r| r["action"] == "eco").unwrap()["uuid"]
        .as_str()
        .unwrap()
        .to_string();
    let ferramenta = spans
        .iter()
        .find(|s| s["name"] == "phxclaw.ferramenta")
        .expect("span da ferramenta");
    assert_eq!(
        attr(ferramenta, "phxclaw.evidencia.id").unwrap()["stringValue"],
        id_ev.as_str()
    );
    assert_eq!(
        attr(ferramenta, "phxclaw.ferramenta.nome").unwrap()["stringValue"],
        "eco"
    );
    assert!(
        spans.iter().any(|s| s["name"] == "phxclaw.modelo"),
        "sem span de modelo"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn traces_do_fluxo_com_passos_e_a_tarefa_filha() {
    let (base, recebidos) = coletor().await;
    let raiz = tmp("fluxo");
    let s = estado(&raiz);
    let arq = raiz.join("f.json");
    std::fs::write(
        &arq,
        json!({"nome": "relatorio", "passos": [
            {"id": "coleta", "ferramenta": "eco", "args": {"texto": SENSIVEL}},
            {"id": "resume", "depende": ["coleta"], "tarefa": "resuma {{coleta}}"}
        ]})
        .to_string(),
    )
    .unwrap();
    let c =
        phxclaw_agent::api::criar_fluxo_com(&s, arq.to_str().unwrap(), vec![], |_| Ok(())).unwrap();
    let t = c.fim.await.unwrap();
    assert!(t.status.is_final(), "{t:#?}");
    let e = exportador(&base);
    e.exportar_tarefa(&s.store, &t.id)
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let corpo = recebidos.lock().unwrap()[0].2.clone();
    let spans = conferir_traces(&corpo, &uuid_hex(&t.id));
    let fluxo = spans.iter().find(|s| s["name"] == "phxclaw.fluxo").unwrap();
    assert_eq!(
        attr(fluxo, "phxclaw.fluxo.nome").unwrap()["stringValue"],
        "relatorio"
    );
    let passos: Vec<String> = fluxo["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            e["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["key"] == "phxclaw.passo.id")
                .unwrap()["value"]["stringValue"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(passos, ["coleta", "resume"]);
    // O passo de agente e uma tarefa filha: span proprio, filho do fluxo, com o id do passo.
    let filha = spans
        .iter()
        .find(|s| s["name"] == "phxclaw.tarefa" && attr(s, "phxclaw.passo.id").is_some())
        .expect("span da tarefa filha");
    assert_eq!(filha["parentSpanId"], fluxo["spanId"]);
    assert!(
        spans
            .iter()
            .any(|s| s["name"] == "phxclaw.ferramenta" && s["parentSpanId"] == fluxo["spanId"]),
        "a chamada do passo de ferramenta nao saiu"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metricas_no_otlp_json() {
    let (base, recebidos) = coletor().await;
    let m: &'static Metricas = Box::leak(Box::new(Metricas::nova()));
    m.definir(true);
    m.fim_de_tarefa(|| FimDeTarefa {
        estado: TaskStatus::Failed,
        passos: 3,
        duracao: Some(Duration::from_millis(1500)),
        custo: Some(0.25),
    });
    m.fim_de_tarefa(|| FimDeTarefa {
        estado: TaskStatus::Completed,
        passos: 1,
        duracao: Some(Duration::from_secs(40)),
        custo: None,
    });
    let e = exportador(&base);
    e.exportar_metricas(m).unwrap().await.unwrap().unwrap();
    let r = recebidos.lock().unwrap().clone();
    assert_eq!(r[0].0, "metrics");
    let rm = &r[0].2["resourceMetrics"][0];
    assert!(chaves(&rm["resource"]["attributes"]).contains(&"service.name".to_string()));
    let ms = rm["scopeMetrics"][0]["metrics"].as_array().unwrap();
    let achar = |n: &str| {
        ms.iter()
            .find(|x| x["name"] == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    let tarefas = &achar("phxclaw_tarefas_total")["sum"];
    assert_eq!(tarefas["aggregationTemporality"], 2);
    assert_eq!(tarefas["isMonotonic"], true);
    let falhas = tarefas["dataPoints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["attributes"][0]["value"]["stringValue"] == "failed")
        .unwrap();
    assert_eq!(falhas["asInt"], "1", "int64 vai como texto no JSON do OTLP");
    let custo = achar("phxclaw_tarefa_custo_total")["sum"]["dataPoints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["attributes"][0]["value"]["stringValue"] == "failed")
        .unwrap()
        .clone();
    assert_eq!(custo["asDouble"], 0.25);
    let h = &achar("phxclaw_tarefa_duracao_segundos")["histogram"]["dataPoints"][0];
    let limites = h["explicitBounds"].as_array().unwrap();
    let faixas: Vec<u64> = h["bucketCounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(
        faixas.len(),
        limites.len() + 1,
        "uma faixa a mais, a do +Inf"
    );
    assert_eq!(h["count"], "2");
    assert_eq!(
        faixas.iter().sum::<u64>(),
        2,
        "contagem POR faixa, nao acumulada: {faixas:?}"
    );
    assert_eq!(achar("phxclaw_tarefa_duracao_segundos")["unit"], "s");
    for x in ms {
        for p in x["sum"]["dataPoints"].as_array().unwrap_or(&vec![]) {
            for k in chaves(&p["attributes"]) {
                assert!(ATRIBUTOS.contains(&k.as_str()), "{k}");
            }
        }
    }
}

/// O painel pelo fio, atras do RBAC: o membro do projeto `alfa` so ve as execucoes de
/// `alfa`; as contas (estado, p50/p95, custo, falha mais comum, por fluxo e por dia) batem.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn insights_contam_o_disco_cortado_pelo_projeto() {
    let raiz = tmp("insights");
    let s = estado(&raiz);
    let agora = chrono::Utc::now();
    let grava = |obj: &str,
                 estado: TaskStatus,
                 ms: i64,
                 projeto: &str,
                 erro: Option<&str>,
                 dias_atras: i64| {
        let mut t = Task::new(obj, "roteiro");
        t.status = estado;
        t.created_at =
            agora - chrono::Duration::days(dias_atras) - chrono::Duration::milliseconds(ms);
        t.updated_at = t.created_at + chrono::Duration::milliseconds(ms);
        t.projeto = Some(projeto.into());
        t.error = erro.map(str::to_string);
        s.store.save(&t).unwrap();
        t
    };
    for ms in [100, 200, 300, 400] {
        grava("fluxo: vendas", TaskStatus::Completed, ms, "alfa", None, 0);
    }
    grava(
        "fluxo: vendas",
        TaskStatus::Failed,
        1000,
        "alfa",
        Some("HTTP 503 em https://api/x 12 vezes"),
        0,
    );
    grava(
        "fluxo: vendas",
        TaskStatus::Failed,
        2000,
        "alfa",
        Some("HTTP 502 em https://api/x 3 vezes"),
        1,
    );
    grava(
        "fluxo: estoque",
        TaskStatus::Failed,
        50,
        "alfa",
        Some("chave ghp_0123456789abcdefghijklmnopqrstuvwxyzAB vazou"),
        0,
    );
    grava(OBJETIVO, TaskStatus::Running, 10, "alfa", None, 0);
    // Fora da janela de 7 dias, e de outro projeto: nenhuma das duas entra.
    grava(
        "fluxo: vendas",
        TaskStatus::Failed,
        10,
        "alfa",
        Some("velha"),
        20,
    );
    for _ in 0..5 {
        grava(
            "fluxo: segredo-de-beta",
            TaskStatus::Failed,
            10,
            "beta",
            Some("de beta"),
            0,
        );
    }
    let token = {
        let a = vec![
            "criar".to_string(),
            "ana".into(),
            "--papel".into(),
            "leitor".into(),
            "--projeto".into(),
            "alfa".into(),
        ];
        rbac::comando(&raiz, &a)
            .unwrap()
            .lines()
            .find(|l| l.starts_with(rbac::PREFIXO_TOKEN))
            .unwrap()
            .to_string()
    };
    let s = ApiState {
        usuarios: Usuarios::da_pasta(&raiz),
        ..s
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, router(s)).await.unwrap() });
    let http = reqwest::Client::new();
    let pedir = |q: &str| {
        http.get(format!("{base}/v1/insights{q}"))
            .bearer_auth(&token)
            .header("X-PhxClaw-Projeto", "alfa")
            .send()
    };
    let r = pedir("?periodo=7d").await.unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["total"], 8, "{v:#}");
    assert_eq!(v["por_estado"]["completed"], 4);
    assert_eq!(v["por_estado"]["failed"], 3);
    assert_eq!(v["por_estado"]["running"], 1);
    assert_eq!(
        v["duracao"]["amostras"], 7,
        "so execucao terminada tem duracao"
    );
    assert_eq!(v["duracao"]["p50_ms"], 300.0);
    assert_eq!(v["duracao"]["p95_ms"], 2000.0);
    let falhas = v["falhas_comuns"].as_array().unwrap();
    assert_eq!(falhas[0]["n"], 2, "{falhas:#?}");
    assert_eq!(falhas[0]["motivo"], "HTTP # em https://api/x # vezes");
    assert!(falhas.iter().any(|f| f["omitido"] == true), "{falhas:#?}");
    assert!(
        !v.to_string().contains("ghp_"),
        "motivo com forma de credencial saiu"
    );
    assert!(
        !v.to_string().contains("beta"),
        "o painel mostrou execucao de outro projeto"
    );
    let vendas = v["fluxos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["nome"] == "vendas")
        .unwrap();
    assert_eq!(vendas["total"], 6);
    assert_eq!(vendas["falhas"], 2);
    assert!((vendas["taxa_falha"].as_f64().unwrap() - 2.0 / 6.0).abs() < 1e-9);
    assert_eq!(v["dias"].as_array().unwrap().len(), 2);
    // Filtro por fluxo e periodo invalido.
    let so: Value = pedir("?periodo=tudo&fluxo=vendas")
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(so["total"], 7);
    assert_eq!(pedir("?periodo=ontem").await.unwrap().status(), 400);
    // Sem token: o portao barra antes do painel.
    let r = http
        .get(format!("{base}/v1/insights"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let _ = std::fs::remove_dir_all(&raiz);
}
