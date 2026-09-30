//! Ferramentas reais sob o motor: navegador (Chromium de verdade), documentos e politica.

use phxclaw_agent::adaptadores::{BrowserSessions, browser_tools, office_tools};
use phxclaw_agent::*;
use phxclaw_browser::BrowserPolicy;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

fn store() -> TaskStore {
    TaskStore::new(std::env::temp_dir().join(format!("phx-adapt-{}", phxclaw_types::new_uuid_v7())))
        .unwrap()
}

/// Servidor com uma pagina de precos e um formulario de busca.
fn site() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let pedido = String::from_utf8_lossy(&buf[..n]).to_string();
            let corpo = if pedido.starts_with("GET /busca?q=") {
                let q = pedido
                    .split("q=")
                    .nth(1)
                    .unwrap()
                    .split(' ')
                    .next()
                    .unwrap()
                    .to_string();
                format!(
                    "<html><head><title>Resultado</title></head><body><h1>Voce buscou: {q}</h1></body></html>"
                )
            } else {
                "<html><head><title>Precos PhxClaw</title></head><body><h1>Tabela de precos</h1>\
                 <table><tr><td>Plano Basico</td><td>49</td></tr><tr><td>Plano Pro</td><td>149</td></tr></table>\
                 <form action='/busca'><input id='q' name='q'><button>ir</button></form></body></html>".to_string()
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}", corpo.len()).as_bytes());
        }
    });
    base
}

#[tokio::test]
async fn agente_le_site_real_no_chromium_preenche_formulario_e_gera_planilha() {
    if phxclaw_browser::find_chromium().is_none() {
        eprintln!("chromium ausente: pulado");
        return;
    }
    let base = site();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "browser_open", json!({"url": format!("{base}/")})),
        ScriptedLlm::call(
            "c2",
            "browser_type",
            json!({"selector": "#q", "text": "phoenix", "submit": true}),
        ),
        ScriptedLlm::call("c3", "browser_screenshot", json!({"path": "tela.png"})),
        ScriptedLlm::call(
            "c4",
            "create_spreadsheet",
            json!({"path": "precos.xlsx", "sheets": [{"name": "Precos", "rows": [["Plano", "Preco"], ["Basico", 49], ["Pro", 149], ["Total", "=SUM(B2:B3)"]]}]}),
        ),
        ScriptedLlm::text("Planilha precos.xlsx criada."),
    ]));
    let sessoes = BrowserSessions::new(BrowserPolicy::only([base.clone()]));
    let mut tools = browser_tools(sessoes.clone());
    tools.extend(office_tools());
    let s = store();
    let a = Agent::new(
        llm.clone(),
        tools,
        AgentConfig::default().grant(&["web.browse", "doc.write"]),
        s.clone(),
    );
    let t = a
        .run(
            Task::new("precos", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.steps);
    let vistos = llm.seen.lock().unwrap().clone();
    // 1) leu o site real renderizado pelo Chromium
    let leitura = &vistos[1].0.last().unwrap().content;
    assert!(
        leitura.contains("Precos PhxClaw") && leitura.contains("149"),
        "{leitura}"
    );
    // 2) digitou e enviou o formulario: a pagina seguinte mostra o termo
    let depois = &vistos[2].0.last().unwrap().content;
    assert!(depois.contains("Voce buscou: phoenix"), "{depois}");
    // 3) captura PNG e planilha viraram artefatos, e a planilha e legivel de volta
    let w = s.workdir(&t.id);
    assert_eq!(
        &std::fs::read(w.join("tela.png")).unwrap()[..8],
        b"\x89PNG\r\n\x1a\n"
    );
    let wb = phxclaw_office::read_xlsx_values(w.join("precos.xlsx")).unwrap();
    assert_eq!(wb.sheets[0].rows.len(), 4);
    assert_eq!(t.artifacts.len(), 2);
    // 4) a sessao de navegador fechou no fim da tarefa (nada de Chromium orfao)
    assert_eq!(sessoes.open_sessions().await, 0);
}

#[tokio::test]
async fn navegador_do_agente_nao_alcanca_rede_interna() {
    if phxclaw_browser::find_chromium().is_none() {
        return;
    }
    let interno = site();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "browser_open", json!({"url": format!("{interno}/")})),
        ScriptedLlm::call(
            "c2",
            "browser_open",
            json!({"url": "http://169.254.169.254/latest/meta-data"}),
        ),
        ScriptedLlm::text("nao consegui"),
    ]));
    // politica do agente real: internet publica sim, rede interna nao
    let pol = BrowserPolicy {
        allowed_origins: vec![],
        allow_any_public: true,
        block_private_networks: true,
    };
    let a = Agent::new(
        llm,
        browser_tools(BrowserSessions::new(pol)),
        AgentConfig::default().grant(&["web.browse"]),
        store(),
    );
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    let saidas: Vec<_> = t
        .steps
        .iter()
        .filter(|p| p.tool.is_some())
        .map(|p| p.outcome.as_str())
        .collect();
    assert_eq!(saidas, vec!["negado", "negado"], "{:?}", t.steps);
}

#[tokio::test]
async fn documento_e_apresentacao_saem_legiveis() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "create_document",
            json!({"path": "rel.docx", "title": "Relatório", "blocks": [{"type": "heading", "level": 1, "text": "Conclusão & próximos passos"}, {"type": "bullets", "items": ["ação um", "ação dois"]}]}),
        ),
        ScriptedLlm::call(
            "c2",
            "create_presentation",
            json!({"path": "deck.pptx", "title": "PhxClaw", "slides": [{"title": "Visão", "bullets": ["agente local"]}]}),
        ),
        ScriptedLlm::call("c3", "read_document", json!({"path": "rel.docx"})),
        ScriptedLlm::text("ok"),
    ]));
    let s = store();
    let a = Agent::new(
        llm.clone(),
        office_tools(),
        AgentConfig::default().grant(&["doc.write", "fs.read"]),
        s.clone(),
    );
    let t = a
        .run(
            Task::new("docs", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.steps);
    let lido = &llm.seen.lock().unwrap()[3]
        .0
        .last()
        .unwrap()
        .content
        .clone();
    assert!(
        lido.contains("Conclusão & próximos passos") && lido.contains("ação dois"),
        "{lido}"
    );
    assert_eq!(
        &std::fs::read(s.workdir(&t.id).join("deck.pptx")).unwrap()[..2],
        b"PK"
    );
}
