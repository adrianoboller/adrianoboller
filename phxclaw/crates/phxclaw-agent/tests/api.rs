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
async fn historico_e_progresso_no_fio() {
    // A trilha de estados sai do caminho real do motor (nao de um helper chamado a parte):
    // uma transicao que escape do `mudar_estado` some desta lista.
    let (base, _) = subir(vec![]).await;
    let id = cli()
        .post(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .json(&json!({"objective": "escreva um relatorio"}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let t = esperar(&base, &id, "completed").await;
    let estados: Vec<&str> = t["historico"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["estado"].as_str().unwrap())
        .collect();
    assert_eq!(estados, vec!["pending", "running", "completed"], "{t}");
    assert!(!t["historico"][0]["em"].as_str().unwrap().is_empty());
    // Sem plano e concluida: progresso medido = 100.
    assert_eq!(t["progresso"], 100);
    // A lista (TaskSummary, o que o kanban le) carrega os mesmos campos.
    let l: Value = cli()
        .get(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item = &l.as_array().unwrap()[0];
    assert_eq!(item["progresso"], 100);
    assert_eq!(item["historico"].as_array().unwrap().len(), 3);
    assert!(item["ajustes"].as_array().unwrap().is_empty());
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

// ------------------------------------------------------------- seguranca da tela

/// Confere os cabecalhos de seguranca de UMA resposta, com o cache esperado dela.
fn conferir_blindagem(rota: &str, r: &reqwest::Response, cache: &str) {
    use phxclaw_agent::pwa::CABECALHOS;
    for (nome, valor) in CABECALHOS {
        let visto = r.headers().get(*nome).map(|v| v.to_str().unwrap_or(""));
        assert_eq!(visto, Some(*valor), "{rota}: cabecalho {nome}");
    }
    let visto = r
        .headers()
        .get("cache-control")
        .map(|v| v.to_str().unwrap_or(""));
    assert_eq!(visto, Some(cache), "{rota}: cache-control");
}

/// Toda rota da tela -- e o dado, o 401 do portao e o 404 -- sai com a CSP estrita, nosniff,
/// sem referer, sem camera/microfone, COOP/CORP e sem moldura. Antes de 09/10/2026 so o
/// formulario dos gatilhos e o canvas mandavam CSP; a tela que guarda o token nao mandava
/// nenhuma.
///
/// RED medido (09/10/2026), cada um com `// REPOSTO` e desfeito:
/// - sem o `.layer(... pwa::blindar)` do `api::router`: falha em `/` (sem
///   content-security-policy);
/// - `blindar_cabecalhos` sem o `no-store` de quem nao disse nada: falha em `/v1/tasks`
///   (cache-control ausente);
/// - `servir_arquivo` sem o `no-cache` da casca: falha em `/` (saiu `no-store`: a casca do
///   PWA deixaria de ser revalidavel e o `blindar` estaria decidindo o cache da tela).
#[tokio::test]
async fn toda_rota_da_tela_e_do_dado_sai_com_os_cabecalhos_de_seguranca() {
    let (base, _) = subir(vec![]).await;
    for rota in [
        "/",
        "/index.html",
        "/manifest.webmanifest",
        "/sw.js",
        "/assets/app.js",
        "/assets/inspecao.js",
        "/assets/app.css",
        "/assets/textos.json",
    ] {
        let r = cli().get(format!("{base}{rota}")).send().await.unwrap();
        assert_eq!(r.status(), 200, "{rota}");
        conferir_blindagem(rota, &r, "no-cache");
    }
    // O dado e o que nao e arquivo da tela: no-store.
    let r = cli()
        .get(format!("{base}/v1/tasks"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    conferir_blindagem("/v1/tasks", &r, "no-store");
    let r = cli().get(format!("{base}/v1/tasks")).send().await.unwrap();
    assert_eq!(r.status(), 401);
    conferir_blindagem("/v1/tasks sem token", &r, "no-store");
    for rota in ["/assets/nao-existe.js", "/rota/que/nao/existe", "/health"] {
        let r = cli().get(format!("{base}{rota}")).send().await.unwrap();
        conferir_blindagem(rota, &r, "no-store");
    }
    // A politica da tela: publica (sem token) e pelo leitor unico do config.
    let r = cli()
        .get(format!("{base}{}", phxclaw_agent::pwa::ROTA_POLITICA))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    conferir_blindagem("/ui/politica", &r, "no-store");
    let p: Value = r.json().await.unwrap();
    assert_eq!(
        p,
        json!({"bloquear_inspecao": phxclaw_agent::pwa::bloquear_inspecao()})
    );
}

/// A CSP precisa de forma, nao so de presenca: as diretivas que separam XSS de texto
/// inofensivo. Quem afrouxar o `script-src` (ou abrir `object-src`/moldura) reprova aqui.
/// RED medido: `script-src 'self' 'unsafe-inline'` (`// REPOSTO`) reprova na primeira linha
/// (e o teste do desktop junto, porque o tauri.conf.json deixou de ser igual).
#[test]
fn a_csp_da_tela_fecha_script_em_linha_objeto_base_e_moldura() {
    let csp = diretivas(phxclaw_agent::pwa::CSP);
    assert_eq!(csp["script-src"], vec!["'self'"]);
    assert_eq!(csp["default-src"], vec!["'self'"]);
    assert_eq!(csp["object-src"], vec!["'none'"]);
    assert_eq!(csp["base-uri"], vec!["'none'"]);
    assert_eq!(csp["frame-ancestors"], vec!["'none'"]);
    assert_eq!(csp["form-action"], vec!["'self'"]);
    assert_eq!(csp["connect-src"], vec!["'self'"]);
    for (d, fontes) in &csp {
        assert!(
            !fontes.iter().any(|f| f == "'unsafe-eval'" || f == "*"),
            "{d}: {fontes:?}"
        );
    }
    // O bloqueio do inspetor nasce LIGADO (pedido do dono), pela fabrica de config.
    let k = phxclaw_agent::config::catalogo_do_config::por_chave("ui.bloquear_inspecao")
        .expect("ui.bloquear_inspecao no catalogo");
    assert_eq!(k.padrao, Some("true"));
    // Camera e localizacao fechadas (a tela nao usa); o microfone fica liberado SO para a
    // propria origem, porque a Conversa grava voz pela tela (getUserMedia), e nunca aberto.
    for f in ["camera=()", "geolocation=()"] {
        assert!(phxclaw_agent::pwa::PERMISSOES.contains(f), "{f}");
    }
    assert!(phxclaw_agent::pwa::PERMISSOES.contains("microphone=(self)"));
    assert!(!phxclaw_agent::pwa::PERMISSOES.contains("microphone=()"));
    assert!(!phxclaw_agent::pwa::PERMISSOES.contains("microphone=(*)"));
    assert!(!phxclaw_agent::pwa::PERMISSOES.contains("microphone=*"));
}

fn diretivas(csp: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    csp.split(';')
        .filter_map(|d| {
            let mut p = d.split_whitespace();
            let nome = p.next()?.to_string();
            Some((nome, p.map(String::from).collect()))
        })
        .collect()
}

/// O desktop (Tauri) com a MESMA CSP do servidor -- so o canal do host (`ipc:`) a mais --, sem
/// a permissao que deixa a pagina abrir o inspetor e sem a feature `devtools` do tauri (sem
/// ela o build de release nao tem DevTools: `tauri-runtime-wry` so chama `with_devtools`
/// sob `debug_assertions` ou essa feature, e o wry nasce com `devtools: false` no release).
///
/// RED medido (09/10/2026), cada um com `// REPOSTO` e desfeito:
/// - `"permissions": ["core:default"]` de volta: reprova (o `core:default` traz o
///   `core:webview:default`, que traz o `allow-internal-toggle-devtools`);
/// - `style-src 'self'` no tauri.conf.json (a CSP de antes): reprova na diretiva style-src.
#[test]
fn a_csp_do_desktop_e_a_do_servidor_e_o_inspetor_fica_fora() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/phxclaw-desktop/src-tauri");
    let conf: Value =
        serde_json::from_str(&std::fs::read_to_string(raiz.join("tauri.conf.json")).unwrap())
            .unwrap();
    let desktop = diretivas(conf["app"]["security"]["csp"].as_str().unwrap());
    let servidor = diretivas(phxclaw_agent::pwa::CSP);
    assert_eq!(
        desktop.keys().collect::<Vec<_>>(),
        servidor.keys().collect::<Vec<_>>(),
        "as diretivas do desktop e do servidor divergem"
    );
    for (d, fontes) in &servidor {
        let mut esperado = fontes.clone();
        if d == "connect-src" {
            esperado.extend(["ipc:".to_string(), "http://ipc.localhost".to_string()]);
        }
        assert_eq!(&desktop[d], &esperado, "{d}");
    }
    let cap: Value = serde_json::from_str(
        &std::fs::read_to_string(raiz.join("capabilities/default.json")).unwrap(),
    )
    .unwrap();
    let perms: Vec<&str> = cap["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for proibida in [
        "core:default",
        "core:webview:default",
        "core:webview:allow-internal-toggle-devtools",
    ] {
        assert!(!perms.contains(&proibida), "{proibida} nas capabilities");
    }
    // A feature `devtools` do tauri ligaria o inspetor no release.
    let cargo = std::fs::read_to_string(raiz.join("Cargo.toml")).unwrap();
    let linha_tauri = cargo
        .lines()
        .find(|l| l.trim_start().starts_with("tauri ="))
        .expect("dependencia tauri");
    assert!(!linha_tauri.contains("devtools"), "{linha_tauri}");
}

/// M5 (09/10/2026): TODA rota do `servir` -- as da API e as que ele junta por fora do
/// portao (gatilhos, formulario, retomada de espera) -- sai com os cabecalhos de seguranca.
/// A lista sai da tabela `rotas::ROTAS` (que a catraca `toda_rota_do_fonte_esta_na_tabela`
/// cruza com o fonte): rota nova entra no teste sozinha. O formulario guarda a CSP DELE
/// (a do hash do estilo vence a geral) e ganha o `X-Frame-Options: DENY` que o
/// `frame-ancestors 'none'` dela pede.
///
/// RED medido, cada um com o defeito reposto e desfeito:
/// - `api::servidor` com o `merge` dos `extras` DEPOIS do `.layer(blindar)`: falha no
///   `POST /v1/triggers/{nome}` (sem nosniff);
/// - `blindar_cabecalhos` pulando o XFO de toda rota com CSP propria (a regra antiga):
///   falha no `GET /v1/triggers/{nome}` (o formulario sem X-Frame-Options).
#[tokio::test]
async fn toda_rota_da_tabela_sai_blindada_inclusive_as_que_o_servir_junta() {
    use phxclaw_agent::gatilhos::{self, GatilhoDeWebhook, Gatilhos};
    use phxclaw_agent::rotas::ROTAS;
    let (_, state) = subir(vec![]).await;
    let dir = std::env::temp_dir().join(format!("phx-api-m5-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let fluxo = dir.join("contato.json");
    std::fs::write(
        &fluxo,
        json!({"nome":"contato","formulario":{"titulo":"Fale","campos":[{"nome":"nome"}]},
               "passos":[{"id":"a","tarefa":"t"}]})
        .to_string(),
    )
    .unwrap();
    let g = Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "contato".into(),
            objetivo: String::new(),
            fluxo: Some(fluxo.to_string_lossy().into_owned()),
            segredo: None,
            segredo_formulario: Some("codigo-do-formulario-123456".into()),
        }],
    };
    let app = phxclaw_agent::api::servidor(
        state.clone(),
        [gatilhos::router(state.clone(), Arc::new(g))],
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let mut falhas = Vec::new();
    for r in ROTAS {
        // O molde vira um caminho concreto: `{nome}` e o gatilho do formulario, o resto
        // um segmento qualquer.
        let caminho: String = r
            .caminho
            .split('/')
            .map(|seg| match seg {
                "{nome}" => "contato",
                s if s.starts_with("{*") => "a/b",
                s if s.starts_with('{') => "x",
                s => s,
            })
            .collect::<Vec<_>>()
            .join("/");
        let metodo = reqwest::Method::from_bytes(r.metodo.as_bytes()).unwrap();
        let resp = http
            .request(metodo, format!("{base}{caminho}"))
            .send()
            .await
            .unwrap_or_else(|e| panic!("{} {caminho}: {e}", r.metodo));
        let h = resp.headers();
        let ver = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).map(str::to_string);
        let rotulo = format!("{} {} ({})", r.metodo, r.caminho, resp.status());
        for n in [
            "x-content-type-options",
            "content-security-policy",
            "referrer-policy",
            "cache-control",
        ] {
            if ver(n).is_none() {
                falhas.push(format!("{rotulo}: sem {n}"));
            }
        }
        let csp = ver("content-security-policy").unwrap_or_default();
        if csp.contains("frame-ancestors 'none'")
            && ver("x-frame-options").as_deref() != Some("DENY")
        {
            falhas.push(format!(
                "{rotulo}: frame-ancestors 'none' sem X-Frame-Options DENY"
            ));
        }
    }
    assert!(
        falhas.is_empty(),
        "{} rota(s) sem blindagem:\n{}",
        falhas.len(),
        falhas.join("\n")
    );
    // O formulario: 200, a CSP DELE (nao a geral) e a moldura proibida nos dois cabecalhos.
    let r = http
        .get(format!("{base}/v1/triggers/contato"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let h = r.headers();
    assert_eq!(
        h["content-security-policy"].to_str().unwrap(),
        gatilhos::csp_do_formulario()
    );
    assert_eq!(h["x-frame-options"], "DENY");
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert_eq!(h["cache-control"], "no-store");
    let _ = std::fs::remove_dir_all(&dir);
}
