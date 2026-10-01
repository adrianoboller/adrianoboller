//! O SDK contra o servidor de verdade: o `router` da API numa porta local, com o modelo de
//! roteiro dos testes do agente. Nada de servidor de mentira -- o que o SDK le e o que a
//! rota escreve.

use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::{Agenda, Agent, AgentConfig, ScriptedLlm, TaskStore, WriteFileTool};
use phxclaw_agent_core::Tool;
use phxclaw_sdk::{Cliente, Erro, NovaTarefa, TaskStatus};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

async fn subir(por_minuto: u32) -> String {
    let dir = std::env::temp_dir().join(format!("phx-sdk-{}", phxclaw_types::new_uuid_v7()));
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |modelo: &str| {
        if modelo == "inexistente" {
            return Err("modelo desconhecido".into());
        }
        let llm = Arc::new(ScriptedLlm::new(vec![
            ScriptedLlm::call(
                "c1",
                "write_file",
                json!({"path": "saida/relatorio.md", "content": "# ok"}),
            ),
            ScriptedLlm::text("pronto: saida/relatorio.md"),
        ]));
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool)];
        Ok(Agent::new(
            llm,
            tools,
            AgentConfig::default().grant(&["fs.write"]),
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
        limite: Arc::new(Limite::por_minuto(por_minuto)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    base
}

fn pedido(objetivo: &str) -> NovaTarefa {
    NovaTarefa {
        objective: objetivo.into(),
        ..NovaTarefa::default()
    }
}

/// O principal: criar, acompanhar ate o fim, ler a resposta, o artefato e a listagem --
/// tudo nos tipos que o servidor serializa.
#[tokio::test]
async fn cria_acompanha_e_le_o_resultado() {
    let base = subir(1000).await;
    let c = Cliente::new(&base, TOKEN).unwrap();
    assert!(c.saude().await.unwrap());
    let id = c.criar(&pedido("escreva o relatorio")).await.unwrap();
    let t = c.aguardar(&id, Duration::from_secs(20)).await.unwrap();
    assert_eq!(t.status, TaskStatus::Completed, "{t:?}");
    assert_eq!(t.answer.as_deref(), Some("pronto: saida/relatorio.md"));
    assert!(
        t.steps
            .iter()
            .any(|p| p.tool.as_deref() == Some("write_file"))
    );
    let a = t
        .artifacts
        .iter()
        .find(|a| a.path.ends_with("relatorio.md"))
        .unwrap_or_else(|| panic!("sem artefato: {t:?}"));
    assert_eq!(c.artefato(&id, &a.path).await.unwrap(), b"# ok");

    let l = c.listar().await.unwrap();
    let r = l.iter().find(|r| r.id == id).unwrap();
    assert_eq!(r.steps, t.steps.len());
    assert_eq!(r.status, TaskStatus::Completed);
    // O token nao vaza pelo Debug.
    assert!(!format!("{c:?}").contains(TOKEN));
}

#[tokio::test]
async fn recusas_chegam_com_codigo_e_mensagem() {
    let base = subir(1).await;
    let errado = Cliente::new(&base, "token-errado-mas-comprido-o-bastante").unwrap();
    match errado.criar(&pedido("x")).await {
        Err(Erro::Api { status: 401, .. }) => {}
        outro => panic!("{outro:?}"),
    }
    let c = Cliente::new(&base, TOKEN).unwrap();
    match c.criar(&pedido("   ")).await {
        Err(Erro::Api {
            status: 400, erro, ..
        }) => assert!(erro.contains("objective"), "{erro}"),
        outro => panic!("{outro:?}"),
    }
    let mut p = pedido("x");
    p.model = Some("inexistente".into());
    match c.criar(&p).await {
        Err(Erro::Api {
            status: 400, erro, ..
        }) => assert!(erro.contains("modelo"), "{erro}"),
        outro => panic!("{outro:?}"),
    }
    match c.tarefa("../../etc").await {
        Err(Erro::Api { status: 404, .. }) => {}
        outro => panic!("{outro:?}"),
    }
    // Balde de uma ficha: a segunda criacao volta 429 com o prazo do servidor.
    c.criar(&pedido("primeira")).await.unwrap();
    match c.criar(&pedido("segunda")).await {
        Err(Erro::Api {
            status: 429,
            retry_after: Some(s),
            ..
        }) => assert!(s >= 1),
        outro => panic!("{outro:?}"),
    }
    assert!(matches!(Cliente::new(&base, ""), Err(Erro::SemToken)));
}
