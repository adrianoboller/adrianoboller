//! A dobra de codigo do IDE web (gap `dobra_codigo`): a rota `GET /v1/ide/dobras` devolve o
//! texto e as regioes dobraveis numa resposta so, com a FONTE dita -- o servidor de linguagem
//! (`textDocument/foldingRange`) quando ha um, a reserva por chaves ou por indentacao quando
//! nao ha. Pela mesma leitura confinada do minimapa: sem Bearer 401, fora do projeto 403.
//! A dobra em si (esconder linha) e da tela: `tests/desktop/ide_dobra.mjs`.

use phxclaw_agent::agenda::Agenda;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::motor::Agent;
use phxclaw_agent::tarefa::TaskStore;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const TOKEN: &str = "token-da-dobra-com-mais-de-24-caracteres";

const MAIN_RS: &str = "fn soma(a: i32, b: i32) -> i32 {\n    let c = a + b;\n    // { chave em comentario }\n    c\n}\n\nfn main() {\n    println!(\"{}\", soma(1, 2));\n}\n";

async fn subir(raiz: &std::path::Path) -> String {
    unsafe {
        std::env::set_var("PHXCLAW_PROJETO", raiz.join("projeto"));
    }
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        let llm = Arc::new(phxclaw_agent::ScriptedLlm::new(vec![]));
        Ok(Agent::new(llm, vec![], Default::default(), st2.clone()))
    });
    let state = ApiState {
        usuarios: Default::default(),
        store,
        factory,
        default_model: "falso".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    base
}

async fn dobras(base: &str, arquivo: &str, prazo: u64, token: Option<&str>) -> (u16, Value) {
    let mut r = reqwest::Client::new().get(format!(
        "http://{base}/v1/ide/dobras?arquivo={}&prazo={prazo}",
        url::form_urlencoded::byte_serialize(arquivo.as_bytes()).collect::<String>()
    ));
    if let Some(t) = token {
        r = r.bearer_auth(t);
    }
    let r = r.send().await.unwrap();
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

fn faixas(v: &Value) -> Vec<(u64, u64)> {
    v["regioes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["inicio"].as_u64().unwrap(), r["fim"].as_u64().unwrap()))
        .collect()
}

/// Um teste so (o `PHXCLAW_PROJETO` e do processo): recusa, reserva por chaves e por
/// indentacao, e o servidor de linguagem quando o hospedeiro tem um.
///
/// RED medido: em `ide::dobras`, `Some(r) => ("lsp", r)` trocado por `Some(_) =>
/// crate::dobras::reserva(..)` (`// REPOSTO`) -- a fonte do `.rs` com servidor saiu
/// `chaves` e o teste caiu na asserção da fonte.
#[tokio::test(flavor = "multi_thread")]
async fn as_dobras_vem_do_lsp_ou_da_reserva_dita_e_pela_leitura_confinada() {
    let raiz = std::env::temp_dir().join(format!("phx-ide-dobra-{}", phxclaw_types::new_uuid_v7()));
    let proj = raiz.join("projeto");
    std::fs::create_dir_all(proj.join("src")).unwrap();
    std::fs::write(
        proj.join("Cargo.toml"),
        "[package]\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(proj.join("src/main.rs"), MAIN_RS).unwrap();
    std::fs::write(proj.join("app.js"), "const o = {\n  a: '}',\n  b: 2,\n};\n").unwrap();
    std::fs::write(proj.join("conf.yaml"), "a:\n  b: 1\n  c:\n    d: 2\ne: 3\n").unwrap();
    std::fs::write(raiz.join("fora.rs"), "fn segredo() {\n    1\n}\n").unwrap();
    let base = subir(&raiz).await;

    let (s, _) = dobras(&base, "app.js", 0, None).await;
    assert_eq!(s, 401);
    let (s, v) = dobras(&base, "../fora.rs", 0, Some(TOKEN)).await;
    assert_eq!(s, 403, "{v}");
    assert!(!v.to_string().contains("segredo"), "{v}");

    // Reserva por chaves (.js, a chave dentro do texto nao conta) e por indentacao (.yaml).
    let (s, v) = dobras(&base, "app.js", 0, Some(TOKEN)).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["fonte"], "chaves");
    assert_eq!(faixas(&v), vec![(1, 3)]);
    assert!(v["texto"].as_str().unwrap().starts_with("const o"));
    let (_, v) = dobras(&base, "conf.yaml", 0, Some(TOKEN)).await;
    assert_eq!(v["fonte"], "indentacao");
    assert_eq!(faixas(&v), vec![(1, 4), (3, 4)]);

    // O .rs com prazo 0: a reserva por chaves, sem esperar servidor nenhum.
    let (_, v) = dobras(&base, "src/main.rs", 0, Some(TOKEN)).await;
    assert_eq!(v["fonte"], "chaves");
    assert_eq!(faixas(&v), vec![(1, 5), (7, 9)]);
    assert_eq!(v["linhas"], 9);

    // Com o servidor de linguagem do hospedeiro (rust-analyzer no bwrap): a fonte e o LSP, e a
    // funcao `soma` dobra a partir da linha 1.
    let tem_lsp = phxclaw_agent::arquivos::achar_bwrap().is_some()
        && phxclaw_agent::lsp::servidores_do_hospedeiro()
            .iter()
            .any(|s| s.linguagem == "rust");
    if tem_lsp {
        let (s, v) = dobras(&base, "src/main.rs", 120, Some(TOKEN)).await;
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["fonte"], "lsp", "{v}");
        assert!(
            faixas(&v)
                .iter()
                .any(|&(i, f)| i == 1 && (3..=5).contains(&f)),
            "{v}"
        );
    } else {
        phxclaw_test_support::pulado::pular(
            "rust-analyzer",
            "sem servidor de linguagem: so a reserva foi provada",
        );
    }
    let _ = std::fs::remove_dir_all(&raiz);
}
