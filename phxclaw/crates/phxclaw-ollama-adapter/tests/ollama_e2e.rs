//! E2E contra um servidor Ollama de verdade (provider local, sem credencial).
//! `PHXCLAW_E2E_OLLAMA_URL=http://127.0.0.1:11434/api/ PHXCLAW_E2E_OLLAMA_MODEL=smollm2:135m
//!  PHXCLAW_E2E_OLLAMA_EMBED_MODEL=all-minilm cargo test -p phxclaw-ollama-adapter --test ollama_e2e -- --ignored`

use phxclaw_ollama_adapter::{OllamaClient, OllamaConfig, OllamaError, OllamaMessage};
use serde_json::json;

fn cliente() -> (OllamaClient, String) {
    let url = std::env::var("PHXCLAW_E2E_OLLAMA_URL").expect("PHXCLAW_E2E_OLLAMA_URL");
    let modelo = std::env::var("PHXCLAW_E2E_OLLAMA_MODEL").expect("PHXCLAW_E2E_OLLAMA_MODEL");
    let c = OllamaClient::new(OllamaConfig {
        base_url: url,
        timeout_ms: 120_000,
    })
    .unwrap();
    (c, modelo)
}

#[tokio::test]
#[ignore = "requer servidor Ollama: PHXCLAW_E2E_OLLAMA_URL"]
async fn versao_modelos_chat_e_embed_reais() {
    let (c, modelo) = cliente();
    let v = c.version().await.unwrap();
    assert!(v["version"].as_str().is_some_and(|s| !s.is_empty()), "{v}");

    let tags = c.list_models().await.unwrap();
    let nomes: Vec<_> = tags["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["name"].as_str())
        .collect();
    assert!(
        nomes.contains(&modelo.as_str()),
        "{modelo} fora de {nomes:?}"
    );

    let msgs = [OllamaMessage {
        role: "user".into(),
        content: "Reply with the single word: ok".into(),
        images: vec![],
    }];
    let limite = Some(json!({"temperature": 0, "num_predict": 16}));
    let r = c
        .chat(&modelo, &msgs, None, None, None, limite)
        .await
        .unwrap();
    let texto = r["message"]["content"].as_str().unwrap_or_default();
    assert!(!texto.trim().is_empty(), "resposta vazia: {r}");
    assert_eq!(r["done"], json!(true), "{r}");
    assert!(
        r["eval_count"].as_u64().unwrap_or(0) > 0,
        "nenhum token gerado: {r}"
    );

    // Embedding pede modelo proprio: o de chat responde 501 (o adapter reporta como Api).
    let m_embed =
        std::env::var("PHXCLAW_E2E_OLLAMA_EMBED_MODEL").expect("PHXCLAW_E2E_OLLAMA_EMBED_MODEL");
    let e = c.embed(&m_embed, json!(["phxclaw"])).await.unwrap();
    let dim = e["embeddings"][0].as_array().map(|a| a.len()).unwrap_or(0);
    assert!(dim > 0, "embedding vazio: {e}");
}

#[tokio::test]
#[ignore = "requer servidor Ollama: PHXCLAW_E2E_OLLAMA_URL"]
async fn modelo_inexistente_vira_erro_de_api_com_status() {
    let (c, _) = cliente();
    let msgs = [OllamaMessage {
        role: "user".into(),
        content: "x".into(),
        images: vec![],
    }];
    match c
        .chat("nao-existe-phxclaw:0b", &msgs, None, None, None, None)
        .await
    {
        Err(OllamaError::Api { status, .. }) => assert_eq!(status, 404),
        outro => panic!("esperava Api 404, veio {outro:?}"),
    }
}
