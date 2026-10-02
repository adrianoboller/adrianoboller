//! Prova do exemplo `exemplos/passagens-china` (SP000034): o fluxo em DAG do monitor de
//! passagens roda SEM modelo, so com ferramentas, e cada efeito dele e conferido de fora --
//! o aviso chega num webhook local que o teste sobe, a planilha nasce no disco, a nota vai
//! para a memoria e a segunda corrida compara com a primeira.
//!
//! Os passos de navegador (`consultar_*`, `captura_*`) entram pelo `fluxos::retomar` com
//! a saida gravada em `fixtures/` -- texto REAL que o `browser_open` devolveu do Google
//! Flights em 02/10/2026. Assim a prova nao depende de rede nem de Chromium, e o resto do
//! fluxo (python, memoria, canal, planilha) roda de verdade pelo portao unico.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use phxclaw_agent::api::{
    AgentFactory, ApiState, Limite, disparar_agenda, disparar_agenda_com_handles, fluxo_do_objetivo,
};
use phxclaw_agent::canais::caixa::Caixa;
use phxclaw_agent::canais::http::{Credencial, politica_para};
use phxclaw_agent::canais::webhook::Webhook;
use phxclaw_agent::canais::{Canal, ChannelSendTool, broker_em, guardar_do_canal};
use phxclaw_agent::fluxos::{self, Fluxo, Relatorio, Resultado};
use phxclaw_agent::memoria::{Memoria, MemorySaveTool, MemorySearchTool};
use phxclaw_agent::*;
use phxclaw_agent_core::Tool;
use phxclaw_memory_context::MemoryLimits;
use phxclaw_secret_broker::SecretValue;
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const PASTA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../exemplos/passagens-china"
);

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-passagens-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fluxo_do_exemplo() -> Fluxo {
    fluxos::ler(&std::fs::read_to_string(format!("{PASTA}/fluxo.json")).unwrap()).unwrap()
}

/// O mesmo sha que `fluxos::retomar` confere: da definicao serializada.
fn sha_do_fluxo(f: &Fluxo) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(serde_json::to_string(f).unwrap().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Servidor HTTP local que guarda cada corpo recebido: e o "canal" da prova.
async fn webhook_local() -> (String, Arc<Mutex<Vec<String>>>) {
    async fn receber(State(log): State<Arc<Mutex<Vec<String>>>>, corpo: Bytes) -> &'static str {
        log.lock()
            .unwrap()
            .push(String::from_utf8_lossy(&corpo).to_string());
        "{}"
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new().fallback(receber).with_state(log.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, log)
}

/// O agente da prova: as ferramentas que o fluxo pede, e nenhuma outra. O modelo e um
/// roteiro vazio -- se algum passo pedisse modelo, o teste cairia aqui, de proposito.
fn agente(dir: &Path, canal: Canal) -> Option<Agent> {
    let bwrap = phxclaw_agent::arquivos::achar_bwrap()?;
    let py = phxclaw_agent::python::PythonProjectTool::detectar(bwrap).ok()?;
    let memoria = Memoria::new(dir.join("memoria.jsonl"), "prova", MemoryLimits::default());
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(WriteFileTool),
        Arc::new(ReadFileTool),
        Arc::new(phxclaw_agent::repl::PythonReplTool::new(Arc::new(py))),
        Arc::new(MemorySaveTool {
            memoria: memoria.clone(),
        }),
        Arc::new(MemorySearchTool { memoria }),
        Arc::new(ChannelSendTool { canal }),
    ];
    tools.extend(phxclaw_agent::adaptadores::office_tools());
    Some(Agent::new(
        Arc::new(ScriptedLlm::new(vec![])),
        tools,
        AgentConfig::default().grant(&[
            "fs.read",
            "fs.write",
            "shell.exec",
            "memory.read",
            "memory.write",
            "doc.write",
            "channel.send",
        ]),
        TaskStore::new(dir.join("tasks")).unwrap(),
    ))
}

/// A "corrida anterior" com os passos de navegador ja feitos: e o que `retomar` reaproveita.
fn corrida_com_navegador_pronto(a: &Agent, f: &Fluxo) -> String {
    let mut passos = Vec::new();
    for i in 1..=3 {
        let pagina = std::fs::read_to_string(format!("{PASTA}/fixtures/pagina_{i}.txt")).unwrap();
        for (id, saida) in [
            (format!("consultar_{i}"), pagina),
            (
                format!("captura_{i}"),
                format!("captura salva em captura_{i}.png"),
            ),
        ] {
            passos.push(Resultado {
                id,
                estado: "ok".into(),
                saida,
                tarefa: None,
                tentativas: 1,
                reaproveitado: false,
            });
        }
    }
    let mut t = Task::new(format!("fluxo: {}", f.nome), a.llm.id());
    t.answer = serde_json::to_string(&Relatorio {
        tarefa: t.id.clone(),
        fluxo_sha256: sha_do_fluxo(f),
        sucesso: false,
        passos,
    })
    .ok();
    a.store.save(&t).unwrap();
    t.id
}

fn saida(r: &Relatorio, id: &str) -> String {
    r.passos
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("passo {id} nao esta no relatorio"))
        .saida
        .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fluxo_extrai_avisa_no_webhook_grava_planilha_e_compara_com_a_memoria() {
    let dir = tmp("fluxo");
    let (url, recebidos) = webhook_local().await;
    let broker = broker_em(&dir).unwrap();
    let id = guardar_do_canal(
        &broker,
        "webhook-segredo",
        "webhook",
        SecretValue::new("segredo-da-prova".into()),
    )
    .unwrap();
    let wh = Arc::new(
        Webhook::novo(
            Caixa::abrir(dir.join("webhook.caixa.jsonl")).unwrap(),
            Credencial::nova(broker, id, "webhook"),
            Some((&url, politica_para(&url).unwrap())),
        )
        .unwrap(),
    );
    // "DONO" e o `to` escrito no fluxo.json: a conversa permitida do canal.
    let canal = Canal::de_arc(
        wh,
        "webhook",
        ["DONO"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        Arc::new(|_: &str| {}),
    )
    .unwrap();
    let Some(a) = agente(&dir, canal) else {
        pulado::pular(
            "bwrap+python",
            "o passo extrair roda no python_repl do sandbox",
        );
        return;
    };
    let f = fluxo_do_exemplo();
    assert_eq!(f.passos.len(), 42);
    assert!(
        f.passos.iter().all(|p| p.tarefa.is_none()),
        "o fluxo e deterministico: nenhum passo de modelo"
    );

    // ---- corrida 1: memoria vazia -------------------------------------------------
    let anterior = corrida_com_navegador_pronto(&a, &f);
    let r = fluxos::retomar(&a, &f, &anterior).await.unwrap();
    let falhas: Vec<_> = r
        .passos
        .iter()
        .filter(|p| p.estado != "ok")
        .map(|p| format!("{}: {} {}", p.id, p.estado, p.saida))
        .collect();
    assert!(r.sucesso, "{falhas:#?}");
    assert!(
        r.passos
            .iter()
            .filter(|p| p.id.starts_with("consultar_") || p.id.starts_with("captura_"))
            .all(|p| p.reaproveitado),
        "navegador veio da fixture, nao de rede"
    );
    // extracao: fixture 1 e PEK ida 2026-11-01 (Ethiopian R$ 5.188), 2 e PVG ida 2026-11-10
    // (American/Etihad R$ 4.292), 3 e uma pagina sem resultado (sem preco)
    assert_eq!(saida(&r, "c_preco_1"), "5188");
    assert_eq!(saida(&r, "c_companhia_1"), "Ethiopian");
    assert_eq!(saida(&r, "c_data_ida_1"), "2026-11-01");
    assert_eq!(saida(&r, "c_preco_2"), "4292");
    assert_eq!(
        saida(&r, "c_data_ida_2"),
        "2026-11-10",
        "a data vem da pagina, nao do parametro"
    );
    assert_eq!(saida(&r, "c_companhia_2"), "American, Etihad");
    assert_eq!(saida(&r, "c_alerta_2"), "ALERTA", "4292 <= teto 6000");
    assert_eq!(saida(&r, "c_preco_3"), "");
    assert_eq!(saida(&r, "c_alerta_3"), "sem_preco");
    let msg = saida(&r, "mensagem");
    assert!(msg.starts_with("ALERTA passagens GRU -> China"), "{msg}");
    assert!(msg.contains("PEK 2026-11-01: BRL 5188 Ethiopian"), "{msg}");
    assert!(msg.contains("CAN ") && msg.contains(": SEM_PRECO"), "{msg}");
    assert!(
        !msg.contains("ultimo"),
        "primeira corrida nao tem historico: {msg}"
    );
    // o aviso chegou no webhook, assinado pelo canal, com o MESMO texto
    let corpo = {
        let g = recebidos.lock().unwrap();
        assert_eq!(g.len(), 1, "um aviso por corrida: {g:?}");
        serde_json::from_str::<Value>(&g[0]).unwrap()
    };
    assert_eq!(corpo["conversa"], "DONO");
    assert_eq!(corpo["texto"].as_str().unwrap(), msg);
    // a planilha existe, com 3 linhas de dados
    let carimbo = saida(&r, "c_carimbo");
    let xlsx = a
        .store
        .workdir(&r.tarefa)
        .join(format!("saida/passagens-{carimbo}.xlsx"));
    assert!(xlsx.is_file(), "{}", xlsx.display());
    // lida pela MESMA ferramenta que o agente usaria (read_document), nao por um leitor a parte
    let ler = phxclaw_agent::adaptadores::office_tools()
        .into_iter()
        .find(|t| t.spec().name == "read_document")
        .unwrap();
    let ctx = phxclaw_agent_core::ToolContext {
        task_id: r.tarefa.clone(),
        workdir: a.store.workdir(&r.tarefa),
        timeout: std::time::Duration::from_secs(30),
    };
    let lido = ler
        .run(
            json!({"path": format!("saida/passagens-{carimbo}.xlsx")}),
            &ctx,
        )
        .await
        .unwrap()
        .content;
    assert!(
        lido.contains("5188") && lido.contains("Ethiopian"),
        "{lido}"
    );
    assert_eq!(lido.matches("Google Flights").count(), 3, "{lido}");
    // a captura de tela ficou nomeada no relatorio
    assert!(saida(&r, "captura_1").contains("captura_1.png"));

    // ---- corrida 2: a memoria da corrida 1 entra na comparacao -----------------------
    let anterior = corrida_com_navegador_pronto(&a, &f);
    let r2 = fluxos::retomar(&a, &f, &anterior).await.unwrap();
    assert!(r2.sucesso, "{:#?}", r2.passos);
    let msg2 = saida(&r2, "mensagem");
    assert!(
        msg2.contains("PEK 2026-11-01: BRL 5188 Ethiopian ALERTA (teto 6000; ultimo 5188; minimo historico 5188)"),
        "{msg2}"
    );
    assert_eq!(recebidos.lock().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn objetivo_de_fluxo_e_reconhecido_pelo_prefixo() {
    assert_eq!(
        fluxo_do_objetivo("fluxo: exemplos/passagens-china/fluxo.json"),
        Some("exemplos/passagens-china/fluxo.json")
    );
    assert_eq!(fluxo_do_objetivo("  fluxo:x.json"), Some("x.json"));
    assert_eq!(fluxo_do_objetivo("fluxo:"), None);
    assert_eq!(fluxo_do_objetivo("resuma as noticias"), None);
}

/// A agenda dispara o FLUXO (nao uma tarefa de modelo) uma vez por vencimento, pelo mesmo
/// `/v1/schedules` do servidor e pelo mesmo `disparar_agenda` que o laco de 20 s chama.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn agenda_dispara_o_fluxo_uma_vez_e_anota_a_tarefa() {
    let dir = tmp("agenda");
    let arq = dir.join("fluxo-minimo.json");
    std::fs::write(
        &arq,
        json!({"nome":"minimo","passos":[
            {"id":"grava","ferramenta":"write_file","args":{"path":"x.txt","content":"disparado"}},
            {"id":"le","depende":["grava"],"ferramenta":"read_file","args":{"path":"x.txt"}}
        ]})
        .to_string(),
    )
    .unwrap();
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool), Arc::new(ReadFileTool)];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![])),
            tools,
            AgentConfig::default().grant(&["fs.read", "fs.write"]),
            st2.clone(),
        ))
    });
    let state = ApiState {
        store: store.clone(),
        factory,
        default_model: "sem-modelo".into(),
        token: "token-de-teste-com-tamanho-suficiente".into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = phxclaw_agent::api::router(state.clone());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/schedules"))
        .bearer_auth("token-de-teste-com-tamanho-suficiente")
        .json(&json!({"name": "passagens a cada 6 h", "objective": format!("fluxo: {}", arq.display()), "every_seconds": 6 * 3600}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    assert_eq!(disparar_agenda(&state), 0, "ainda nao venceu");
    // adianta o relogio: o intervalo curto da prova
    state.agenda.lock().unwrap().items[0].next_run =
        chrono::Utc::now() - chrono::Duration::seconds(1);
    let handles = disparar_agenda_com_handles(&state);
    assert_eq!(handles.len(), 1);
    for h in handles {
        h.await.unwrap();
    }
    let tarefas = store.list().unwrap();
    assert_eq!(tarefas.len(), 1, "um disparo, uma tarefa de fluxo");
    let t = store.load(&tarefas[0].id).unwrap();
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    assert_eq!(t.objective, "fluxo: minimo");
    let rel: Relatorio = serde_json::from_str(t.answer.as_deref().unwrap()).unwrap();
    assert!(rel.sucesso);
    assert_eq!(saida(&rel, "le"), "disparado");
    let agenda = state.agenda.lock().unwrap();
    assert_eq!(agenda.items[0].last_task.as_deref(), Some(t.id.as_str()));
    assert!(agenda.items[0].next_run > chrono::Utc::now(), "reagendado");
    drop(agenda);
    assert_eq!(disparar_agenda(&state), 0, "dispara UMA vez por vencimento");
    let _ = std::fs::remove_dir_all(dir);
}
