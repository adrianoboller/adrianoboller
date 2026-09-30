//! Prova dos quatro provedores contra servidor HTTP local: o pedido sai no formato de fio
//! exato (metodo, caminho, autenticacao, JSON inteiro) e a resposta realista com chamada de
//! ferramenta volta como `ToolCall`.

mod comum;

use phxclaw_agent_core::{Llm, LlmError, LlmOptions, Message, ToolCall, ToolSpec};
use phxclaw_llm::{AnthropicLlm, Endpoint, GeminiLlm, OllamaLlm, OpenAiLlm};
use serde_json::json;
use std::time::Duration;

const CHAVE: &str = "sk-teste-SEGREDO-0123456789";
const MODELO: &str = "modelo-teste";

fn conversa() -> (Vec<Message>, Vec<ToolSpec>, LlmOptions) {
    let chamada = ToolCall {
        id: "call_1".into(),
        name: "web_search".into(),
        arguments: json!({"query": "rust release"}),
    };
    let msgs = vec![
        Message::system("Voce e um agente."),
        Message::user("Qual a versao do Rust?"),
        Message::assistant("", vec![chamada.clone()]),
        Message::tool_result(&chamada, "Rust 1.90 lancado"),
    ];
    let tools = vec![ToolSpec {
        name: "web_search".into(),
        description: "Busca na web".into(),
        parameters: json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"],
        }),
    }];
    let opts = LlmOptions {
        max_output_tokens: 256,
        temperature: 0.5,
    };
    (msgs, tools, opts)
}

fn local(base: &str) -> Endpoint {
    Endpoint::custom(base, &[base]).permitir_http_loopback()
}

fn schema() -> serde_json::Value {
    conversa().1[0].parameters.clone()
}

// ---------------------------------------------------------------- OpenAI

#[tokio::test]
async fn openai_traduz_ferramenta_nos_dois_sentidos() {
    let resposta = json!({
        "id": "resp_1", "object": "response", "status": "completed", "model": "modelo-teste-2026",
        "output": [
            {"type": "reasoning", "id": "rs_1", "summary": []},
            {"type": "message", "id": "msg_1", "role": "assistant", "status": "completed",
             "content": [{"type": "output_text", "text": "Vou buscar.", "annotations": []}]},
            {"type": "function_call", "id": "fc_1", "call_id": "call_abc", "name": "web_search",
             "arguments": "{\"query\":\"latest rust\"}", "status": "completed"}
        ],
        "usage": {"input_tokens": 42, "output_tokens": 7, "total_tokens": 49}
    });
    let (base, rx) = comum::subir(200, resposta.to_string());
    let llm = OpenAiLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    let (m, t, o) = conversa();
    let r = llm.chat(&m, &t, &o).await.unwrap();

    let p = rx.recv().unwrap();
    assert_eq!(p.metodo, "POST");
    assert_eq!(p.caminho, "/v1/responses");
    assert_eq!(
        p.cabecalho("authorization"),
        Some(&*format!("Bearer {CHAVE}"))
    );
    assert_eq!(
        p.corpo,
        json!({
            "model": MODELO,
            "input": [
                {"role": "system", "content": "Voce e um agente."},
                {"role": "user", "content": "Qual a versao do Rust?"},
                {"type": "function_call", "call_id": "call_1", "name": "web_search",
                 "arguments": "{\"query\":\"rust release\"}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "Rust 1.90 lancado"}
            ],
            "max_output_tokens": 256,
            "temperature": 0.5,
            "store": false,
            "tools": [{"type": "function", "name": "web_search", "description": "Busca na web",
                       "parameters": schema(), "strict": false}]
        })
    );
    assert_eq!(r.content, "Vou buscar.");
    assert_eq!(
        r.tool_calls,
        vec![ToolCall {
            id: "call_abc".into(),
            name: "web_search".into(),
            arguments: json!({"query": "latest rust"}),
        }]
    );
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (42, 7));
    assert_eq!(r.model, "modelo-teste-2026");
}

#[tokio::test]
async fn openai_401_nao_vaza_a_chave() {
    // Pior caso: o servidor ecoa a chave inteira no corpo do erro.
    let corpo = json!({"error": {"message": format!("Incorrect API key provided: {CHAVE}"),
                                 "code": "invalid_api_key"}});
    let (base, _rx) = comum::subir(401, corpo.to_string());
    let llm = OpenAiLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    assert!(!format!("{llm:?}").contains("SEGREDO"));
    let (m, t, o) = conversa();
    let e = llm.chat(&m, &t, &o).await.unwrap_err();
    assert!(matches!(e, LlmError::Api { status: 401, .. }), "{e:?}");
    for texto in [format!("{e}"), format!("{e:?}")] {
        assert!(!texto.contains("SEGREDO"), "{texto}");
        assert!(texto.contains("[REDACTED]"), "{texto}");
    }
}

// ---------------------------------------------------------------- Anthropic

#[tokio::test]
async fn anthropic_traduz_ferramenta_nos_dois_sentidos() {
    let resposta = json!({
        "id": "msg_01", "type": "message", "role": "assistant", "model": "modelo-teste-2026",
        "content": [
            {"type": "text", "text": "Vou buscar."},
            {"type": "tool_use", "id": "toolu_01", "name": "web_search",
             "input": {"query": "latest rust"}}
        ],
        "stop_reason": "tool_use", "stop_sequence": null,
        "usage": {"input_tokens": 50, "output_tokens": 12}
    });
    let (base, rx) = comum::subir(200, resposta.to_string());
    let llm = AnthropicLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    let (m, t, o) = conversa();
    let r = llm.chat(&m, &t, &o).await.unwrap();

    let p = rx.recv().unwrap();
    assert_eq!(p.metodo, "POST");
    assert_eq!(p.caminho, "/v1/messages");
    assert_eq!(p.cabecalho("x-api-key"), Some(CHAVE));
    assert_eq!(p.cabecalho("anthropic-version"), Some("2023-06-01"));
    assert_eq!(p.cabecalho("authorization"), None);
    assert_eq!(
        p.corpo,
        json!({
            "model": MODELO,
            "max_tokens": 256,
            "temperature": 0.5,
            "system": "Voce e um agente.",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "Qual a versao do Rust?"}]},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "call_1",
                    "name": "web_search", "input": {"query": "rust release"}}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_1",
                    "content": "Rust 1.90 lancado"}]}
            ],
            "tools": [{"name": "web_search", "description": "Busca na web",
                       "input_schema": schema()}]
        })
    );
    assert_eq!(r.content, "Vou buscar.");
    assert_eq!(
        r.tool_calls,
        vec![ToolCall {
            id: "toolu_01".into(),
            name: "web_search".into(),
            arguments: json!({"query": "latest rust"}),
        }]
    );
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (50, 12));
}

#[tokio::test]
async fn anthropic_junta_resultados_paralelos_numa_mensagem_user() {
    let (base, rx) = comum::subir(200, json!({"content": [], "usage": {}}).to_string());
    let llm = AnthropicLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    let a = ToolCall {
        id: "a".into(),
        name: "x".into(),
        arguments: json!({}),
    };
    let b = ToolCall {
        id: "b".into(),
        name: "y".into(),
        arguments: json!({}),
    };
    let m = vec![
        Message::user("oi"),
        Message::assistant("", vec![a.clone(), b.clone()]),
        Message::tool_result(&a, "ra"),
        Message::tool_result(&b, "rb"),
    ];
    llm.chat(&m, &[], &LlmOptions::default()).await.unwrap();
    let msgs = rx.recv().unwrap().corpo["messages"].clone();
    assert_eq!(msgs.as_array().unwrap().len(), 3, "{msgs}");
    assert_eq!(msgs[2]["content"].as_array().unwrap().len(), 2);
    assert_eq!(msgs[2]["content"][1]["tool_use_id"], "b");
}

#[tokio::test]
async fn anthropic_401_nao_vaza_a_chave() {
    let corpo = json!({"type": "error", "error": {"type": "authentication_error",
                      "message": format!("invalid x-api-key {CHAVE}")}});
    let (base, _rx) = comum::subir(401, corpo.to_string());
    let llm = AnthropicLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    assert!(!format!("{llm:?}").contains("SEGREDO"));
    let (m, t, o) = conversa();
    let e = llm.chat(&m, &t, &o).await.unwrap_err();
    assert!(matches!(e, LlmError::Api { status: 401, .. }), "{e:?}");
    assert!(!format!("{e} {e:?}").contains("SEGREDO"));
}

// ---------------------------------------------------------------- Gemini

#[tokio::test]
async fn gemini_traduz_ferramenta_nos_dois_sentidos() {
    let resposta = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [
                {"text": "pensando", "thought": true},
                {"text": "Vou buscar."},
                {"functionCall": {"name": "web_search", "args": {"query": "latest rust"}}}
            ]},
            "finishReason": "STOP", "index": 0
        }],
        "usageMetadata": {"promptTokenCount": 30, "candidatesTokenCount": 9, "totalTokenCount": 39},
        "modelVersion": "modelo-teste-001"
    });
    let (base, rx) = comum::subir(200, resposta.to_string());
    let llm = GeminiLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    let (m, t, o) = conversa();
    let r = llm.chat(&m, &t, &o).await.unwrap();

    let p = rx.recv().unwrap();
    assert_eq!(p.metodo, "POST");
    assert_eq!(
        p.caminho,
        format!("/v1beta/models/{MODELO}:generateContent")
    );
    assert_eq!(p.cabecalho("x-goog-api-key"), Some(CHAVE));
    // A chave vai no cabecalho, nunca na URL (onde acabaria em log de proxy).
    assert!(!p.caminho.contains("SEGREDO"));
    assert_eq!(
        p.corpo,
        json!({
            "systemInstruction": {"parts": [{"text": "Voce e um agente."}]},
            "contents": [
                {"role": "user", "parts": [{"text": "Qual a versao do Rust?"}]},
                {"role": "model", "parts": [{"functionCall": {"name": "web_search",
                    "args": {"query": "rust release"}}}]},
                {"role": "user", "parts": [{"functionResponse": {"name": "web_search",
                    "response": {"content": "Rust 1.90 lancado"}}}]}
            ],
            "generationConfig": {"temperature": 0.5, "maxOutputTokens": 256},
            "tools": [{"functionDeclarations": [{"name": "web_search",
                "description": "Busca na web", "parameters": schema()}]}]
        })
    );
    assert_eq!(r.content, "Vou buscar.", "o texto de pensamento nao entra");
    assert_eq!(r.tool_calls.len(), 1);
    assert_eq!(r.tool_calls[0].name, "web_search");
    assert_eq!(r.tool_calls[0].arguments, json!({"query": "latest rust"}));
    assert!(
        r.tool_calls[0].id.starts_with("call_"),
        "id gerado: {}",
        r.tool_calls[0].id
    );
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (30, 9));
    assert_eq!(r.model, "modelo-teste-001");
}

#[tokio::test]
async fn gemini_devolve_a_assinatura_de_pensamento_no_turno_seguinte() {
    let r1 = json!({"candidates": [{"content": {"role": "model", "parts": [
        {"functionCall": {"name": "web_search", "args": {"query": "q"}},
         "thoughtSignature": "SIG-OPACA"}]}}]});
    let r2 = json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "ok"}]}}]});
    let (base, rx) = comum::subir_varios(vec![(200, r1.to_string()), (200, r2.to_string())]);
    let llm = GeminiLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    let rep = llm
        .chat(&[Message::user("busque")], &[], &LlmOptions::default())
        .await
        .unwrap();
    let chamada = rep.tool_calls[0].clone();
    let m = vec![
        Message::user("busque"),
        Message::assistant("", vec![chamada.clone()]),
        Message::tool_result(&chamada, "achei"),
    ];
    let rep2 = llm.chat(&m, &[], &LlmOptions::default()).await.unwrap();
    assert_eq!(rep2.content, "ok");
    let _primeiro = rx.recv().unwrap();
    let segundo = rx.recv().unwrap();
    assert_eq!(
        segundo.corpo["contents"][1],
        json!({"role": "model", "parts": [{"functionCall": {"name": "web_search",
            "args": {"query": "q"}}, "thoughtSignature": "SIG-OPACA"}]})
    );
}

#[tokio::test]
async fn gemini_401_nao_vaza_a_chave() {
    let corpo = json!({"error": {"code": 401, "message": format!("API key not valid: {CHAVE}"),
                                 "status": "UNAUTHENTICATED"}});
    let (base, _rx) = comum::subir(401, corpo.to_string());
    let llm = GeminiLlm::new(CHAVE.into(), MODELO, local(&base)).unwrap();
    assert!(!format!("{llm:?}").contains("SEGREDO"));
    let e = llm
        .chat(&[Message::user("oi")], &[], &LlmOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(e, LlmError::Api { status: 401, .. }), "{e:?}");
    assert!(!format!("{e} {e:?}").contains("SEGREDO"));
}

// ---------------------------------------------------------------- politica de origem

#[test]
fn nuvem_recusa_origem_nao_listada_e_http_sem_permissao() {
    for e in [
        OpenAiLlm::new(
            CHAVE.into(),
            MODELO,
            Endpoint::custom("https://outro.com", &[]),
        )
        .unwrap_err(),
        AnthropicLlm::new(
            CHAVE.into(),
            MODELO,
            Endpoint::custom("http://127.0.0.1:9", &["http://127.0.0.1:9"]),
        )
        .unwrap_err(),
        GeminiLlm::new(CHAVE.into(), "../x", Endpoint::official()).unwrap_err(),
    ] {
        assert!(matches!(e, LlmError::Denied(_)), "{e:?}");
        assert!(!format!("{e:?}").contains("SEGREDO"));
    }
}

#[tokio::test]
async fn erro_de_transporte_nao_leva_url_nem_chave() {
    // Porta fechada: o reqwest falha na conexao; a mensagem nao pode citar a URL.
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", ouvinte.local_addr().unwrap());
    drop(ouvinte);
    let llm = OpenAiLlm::with_timeout(CHAVE.into(), MODELO, local(&base), Duration::from_secs(5))
        .unwrap();
    let e = llm
        .chat(&[Message::user("oi")], &[], &LlmOptions::default())
        .await
        .unwrap_err();
    let t = format!("{e} {e:?}");
    assert!(matches!(e, LlmError::Transport(_)), "{t}");
    assert!(!t.contains("127.0.0.1") && !t.contains("SEGREDO"), "{t}");
}

// ---------------------------------------------------------------- Ollama

#[tokio::test]
async fn ollama_traduz_ferramenta_nos_dois_sentidos() {
    let resposta = json!({
        "model": "modelo-teste", "created_at": "2026-09-30T12:00:00Z",
        "message": {"role": "assistant", "content": "",
            "tool_calls": [{"function": {"name": "web_search",
                                         "arguments": {"query": "latest rust"}}}]},
        "done": true, "done_reason": "stop", "prompt_eval_count": 80, "eval_count": 20
    });
    let (base, rx) = comum::subir(200, resposta.to_string());
    let llm = OllamaLlm::new(&base, MODELO).unwrap();
    let (m, t, o) = conversa();
    let r = llm.chat(&m, &t, &o).await.unwrap();

    let p = rx.recv().unwrap();
    assert_eq!(p.metodo, "POST");
    assert_eq!(p.caminho, "/api/chat");
    assert_eq!(
        p.corpo,
        json!({
            "model": MODELO,
            "messages": [
                {"role": "system", "content": "Voce e um agente."},
                {"role": "user", "content": "Qual a versao do Rust?"},
                {"role": "assistant", "content": "", "tool_calls": [{"id": "call_1",
                    "function": {"name": "web_search", "arguments": {"query": "rust release"}}}]},
                {"role": "tool", "content": "Rust 1.90 lancado", "tool_name": "web_search"}
            ],
            "stream": false,
            "options": {"temperature": 0.5, "num_predict": 256},
            "tools": [{"type": "function", "function": {"name": "web_search",
                "description": "Busca na web", "parameters": schema()}}]
        })
    );
    assert_eq!(r.tool_calls.len(), 1);
    assert_eq!(r.tool_calls[0].name, "web_search");
    assert_eq!(r.tool_calls[0].arguments, json!({"query": "latest rust"}));
    assert!(
        r.tool_calls[0].id.starts_with("call_"),
        "sem id no fio, id gerado"
    );
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (80, 20));
}

#[test]
fn ollama_remoto_sem_https_e_recusado() {
    assert!(matches!(
        OllamaLlm::new("http://10.0.0.7:11434", MODELO),
        Err(LlmError::Denied(_))
    ));
    assert!(OllamaLlm::new("https://ollama.exemplo.com", MODELO).is_ok());
}

/// Prova contra o Ollama real. `cargo test -p phxclaw-llm -- --ignored`
#[tokio::test]
#[ignore = "exige Ollama em 127.0.0.1:11434 com qwen2.5:1.5b"]
async fn ollama_real_pede_web_search() {
    let llm = OllamaLlm::new("http://127.0.0.1:11434", "qwen2.5:1.5b").unwrap();
    let tools = vec![ToolSpec {
        name: "web_search".into(),
        description: "Search the web and return result snippets.".into(),
        parameters: json!({"type": "object",
            "properties": {"query": {"type": "string", "description": "search query"}},
            "required": ["query"]}),
    }];
    let m = vec![
        Message::system("You are an agent. Always use tools to get information."),
        Message::user("Search the web for the latest Rust release version."),
    ];
    let r = llm.chat(&m, &tools, &LlmOptions::default()).await.unwrap();
    eprintln!("resposta real: {r:?}");
    let c = r
        .tool_calls
        .iter()
        .find(|c| c.name == "web_search")
        .expect("o modelo tinha de pedir web_search");
    assert!(c.arguments["query"].as_str().is_some_and(|q| !q.is_empty()));
    assert!(!c.id.is_empty());
}
