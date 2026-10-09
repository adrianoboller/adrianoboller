//! API de tarefas pelo fio HTTP de verdade: servidor axum numa porta local, cliente reqwest.

use phxclaw_agent::api::{AgentFactory, ApiState, Limite, disparar_agenda, router};
use phxclaw_agent::*;
use phxclaw_agent_core::{LlmReply, Tool};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

fn roteiro() -> Vec<LlmReply> {
    vec![
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "saida/relatorio.md", "content": "# ok"}),
        ),
        ScriptedLlm::text("pronto: saida/relatorio.md"),
    ]
}

async fn subir(webhooks: Vec<String>) -> (String, ApiState) {
    subir_com(webhooks, 1000).await
}

async fn subir_com(webhooks: Vec<String>, por_minuto: u32) -> (String, ApiState) {
    let dir = std::env::temp_dir().join(format!("phx-api-{}", phxclaw_types::new_uuid_v7()));
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    // Cada tarefa ganha o seu roteiro; o plano (quando pedido) consome uma resposta a mais.
    let factory: AgentFactory = Arc::new(move |modelo: &str| {
        if modelo == "inexistente" {
            return Err("modelo desconhecido".into());
        }
        let mut r = vec![ScriptedLlm::text(
            "{\"steps\": [\"escrever relatorio\", \"responder\"]}",
        )];
        r.extend(roteiro());
        let llm = Arc::new(ScriptedLlm::new(if modelo == "com-plano" {
            r
        } else {
            roteiro()
        }));
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool)];
        Ok(Agent::new(
            llm,
            tools,
            AgentConfig::default().grant(&["fs.write"]),
            st2.clone(),
        ))
    });
    let state = ApiState {
        usuarios: Default::default(),
        store,
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: webhooks,
        limite: Arc::new(Limite::por_minuto(por_minuto)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(state.clone());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, state)
}

fn cli() -> reqwest::Client {
    reqwest::Client::new()
}

async fn esperar(base: &str, id: &str, alvo: &str) -> Value {
    for _ in 0..100 {
        let t: Value = cli()
            .get(format!("{base}/v1/tasks/{id}"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if t["status"] == alvo {
            return t;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("tarefa {id} nao chegou a {alvo}");
}

#[tokio::test]
async fn sem_token_e_401_e_com_token_a_tarefa_roda_e_o_artefato_baixa() {
    let (base, _) = subir(vec![]).await;
    let r = cli()
        .post(format!("{base}/v1/tasks"))
        .json(&json!({"objective": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth("errado")
        .json(&json!({"objective": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);

    let r = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .json(&json!({"objective": "escreva um relatorio"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let id = r.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let t = esperar(&base, &id, "completed").await;
    assert_eq!(t["answer"], "pronto: saida/relatorio.md");
    let a = cli()
        .get(format!("{base}/v1/tasks/{id}/artifacts/saida/relatorio.md"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(a.status(), 200);
    assert_eq!(a.headers()["content-security-policy"], "sandbox");
    assert_eq!(a.text().await.unwrap(), "# ok");
    // artefato que nao e da tarefa, ou caminho de fuga: nao sai
    for p in ["../task.json", "outro.md", "..%2Ftask.json"] {
        let r = cli()
            .get(format!("{base}/v1/tasks/{id}/artifacts/{p}"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap();
        assert_ne!(r.status(), 200, "{p} foi servido");
    }
    let l: Value = cli()
        .get(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(l.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn plan_mode_espera_aprovacao_aceita_edicao_e_so_entao_executa() {
    let (base, _) = subir(vec![]).await;
    let id = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .json(&json!({"objective": "relatorio", "model": "com-plano", "plan_first": true}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let t = esperar(&base, &id, "awaiting_approval").await;
    assert_eq!(t["plan"], json!(["escrever relatorio", "responder"]));
    assert!(
        t["steps"].as_array().unwrap().is_empty(),
        "executou antes de aprovar"
    );
    let t: Value = cli()
        .post(format!("{base}/v1/tasks/{id}/plan"))
        .bearer_auth(TOKEN)
        .json(&json!({"steps": ["so escrever"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(t["plan"], json!(["so escrever"]));
    assert_eq!(
        cli()
            .post(format!("{base}/v1/tasks/{id}/approve"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        202
    );
    esperar(&base, &id, "completed").await;
    // aprovar de novo nao reexecuta
    assert_eq!(
        cli()
            .post(format!("{base}/v1/tasks/{id}/approve"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
}

#[tokio::test]
async fn webhook_fora_da_lista_e_recusado_e_o_permitido_recebe_o_fim() {
    // receptor de webhook local que conta e guarda o corpo
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origem = format!("http://{}", l.local_addr().unwrap());
    let corpo = Arc::new(Mutex::new(String::new()));
    let c2 = corpo.clone();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let mut buf = vec![0u8; 65536];
            let mut lido = 0;
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            while let Ok(n) = s.read(&mut buf[lido..]) {
                if n == 0 {
                    break;
                }
                lido += n;
                if String::from_utf8_lossy(&buf[..lido]).contains("task.finished")
                    && String::from_utf8_lossy(&buf[..lido]).ends_with('}')
                {
                    break;
                }
            }
            *c2.lock().unwrap() = String::from_utf8_lossy(&buf[..lido]).into_owned();
            let _ = s.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
        }
    });
    let (base, _) = subir(vec![origem.clone()]).await;
    let r = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .json(&json!({"objective": "x", "webhook": "http://169.254.169.254/latest"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        400,
        "webhook para metadados de nuvem foi aceito"
    );
    let id = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .json(&json!({"objective": "x", "webhook": format!("{origem}/fim")}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    esperar(&base, &id, "completed").await;
    for _ in 0..40 {
        if !corpo.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let c = corpo.lock().unwrap().clone();
    assert!(c.starts_with("POST /fim"), "{c}");
    assert!(c.contains(&id) && c.contains("\"completed\""), "{c}");
}

#[tokio::test]
async fn modelo_invalido_e_objetivo_vazio_sao_400() {
    let (base, _) = subir(vec![]).await;
    for corpo in [
        json!({"objective": ""}),
        json!({"objective": "x", "model": "inexistente"}),
    ] {
        let r = cli()
            .post(format!("{base}/v1/tasks"))
            .bearer_auth(TOKEN)
            .json(&corpo)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400, "{corpo}");
    }
}

#[tokio::test]
async fn agenda_cria_tarefa_quando_vence() {
    let (base, st) = subir(vec![]).await;
    let r = cli()
        .post(format!("{base}/v1/schedules"))
        .bearer_auth(TOKEN)
        .json(&json!({"name": "a cada minuto", "objective": "relatorio", "every_seconds": 60}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let r = cli()
        .post(format!("{base}/v1/schedules"))
        .bearer_auth(TOKEN)
        .json(&json!({"name": "rapido demais", "objective": "x", "every_seconds": 5}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    // adianta o relogio da agenda: forca o vencimento
    st.agenda.lock().unwrap().items[0].next_run = chrono::Utc::now() - chrono::Duration::seconds(1);
    assert_eq!(disparar_agenda(&st), 1);
    let l: Value = cli()
        .get(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(l.as_array().unwrap().len(), 1);
    assert_eq!(disparar_agenda(&st), 0, "disparou duas vezes");
}

#[tokio::test]
async fn site_so_e_servido_depois_de_publicado_e_com_csp_sandbox() {
    let (base, st) = subir(vec![]).await;
    let id = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .json(&json!({"objective": "x"}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    esperar(&base, &id, "completed").await;
    let w = st.store.workdir(&id);
    std::fs::create_dir_all(w.join("site")).unwrap();
    std::fs::write(w.join("site/index.html"), "<h1>ola</h1>").unwrap();
    // escrito mas nao publicado: 404
    let r = cli()
        .get(format!("{base}/sites/{id}/site/index.html"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    let ctx = phxclaw_agent_core::ToolContext {
        task_id: id.clone(),
        workdir: w.clone(),
        timeout: Duration::from_secs(5),
    };
    let t = phxclaw_agent::site::PublishSiteTool {
        base_url: base.clone(),
    };
    use phxclaw_agent_core::Tool as _;
    let out = t.run(json!({"folder": "site"}), &ctx).await.unwrap();
    assert!(
        out.content
            .ends_with(&format!("/sites/{id}/site/index.html"))
    );
    let r = cli()
        .get(format!("{base}/sites/{id}/site/"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.headers()["content-security-policy"],
        "sandbox allow-scripts allow-forms"
    );
    assert_eq!(r.text().await.unwrap(), "<h1>ola</h1>");
    // o resto da pasta da tarefa continua fora
    std::fs::write(w.join("segredo.txt"), "x").unwrap();
    for p in [
        "segredo.txt",
        "site/../segredo.txt",
        "site/%2e%2e/segredo.txt",
    ] {
        assert_ne!(
            cli()
                .get(format!("{base}/sites/{id}/{p}"))
                .send()
                .await
                .unwrap()
                .status(),
            200,
            "{p}"
        );
    }
}

#[tokio::test]
async fn criar_alem_do_limite_e_429_com_retry_after_e_consulta_nao_gasta() {
    let (base, _) = subir_com(vec![], 2).await;
    let criar = |obj: &str| {
        cli()
            .post(format!("{base}/v1/tasks"))
            .bearer_auth(TOKEN)
            .json(&json!({"objective": obj}))
            .send()
    };
    // pedido invalido nao gasta ficha
    assert_eq!(criar("").await.unwrap().status(), 400);
    let a = criar("um").await.unwrap();
    assert_eq!(a.status(), 202);
    let id = a.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    // consultar varias vezes nao gasta ficha
    for _ in 0..5 {
        let r = cli()
            .get(format!("{base}/v1/tasks/{id}"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
    }
    assert_eq!(criar("dois").await.unwrap().status(), 202);
    let r = criar("tres").await.unwrap();
    assert_eq!(r.status(), 429);
    let seg: u64 = r.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=30).contains(&seg), "retry-after {seg}");
}

#[test]
fn balde_repoe_fichas_com_o_tempo() {
    let l = Limite::por_minuto(60); // uma ficha por segundo
    for _ in 0..60 {
        l.tomar().unwrap();
    }
    assert_eq!(l.tomar(), Err(1));
    std::thread::sleep(Duration::from_millis(1100));
    assert!(l.tomar().is_ok());
}

/// O fim conferido pela API: `verificar` sem `shell.exec` no agente e recusado na entrada
/// (seria a porta lateral do shell negado), esquema que nao e objeto tambem; o valido
/// chega a tarefa gravada.
#[tokio::test]
async fn verificar_sem_shell_e_esquema_invalido_se_recusam_na_entrada() {
    let (base, _) = subir(vec![]).await;
    let pedir = |corpo: Value| {
        let base = base.clone();
        async move {
            cli()
                .post(format!("{base}/v1/tasks"))
                .bearer_auth(TOKEN)
                .json(&corpo)
                .send()
                .await
                .unwrap()
        }
    };
    let r = pedir(json!({"objective": "x", "verificar": "cargo test"})).await;
    assert_eq!(r.status(), 400);
    assert!(r.text().await.unwrap().contains("shell.exec"));
    let r = pedir(json!({"objective": "x", "saida_esquema": "objeto"})).await;
    assert_eq!(r.status(), 400);
    let esquema = json!({"type":"object","properties":{"ok":{"type":"boolean"}}});
    let r = pedir(json!({"objective": "x", "saida_esquema": esquema})).await;
    assert_eq!(r.status(), 202);
    let id = r.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let t: Value = cli()
        .get(format!("{base}/v1/tasks/{id}"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(t["saida_esquema"], esquema);
}
