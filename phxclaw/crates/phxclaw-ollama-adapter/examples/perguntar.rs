//! Pergunta ao Ollama local pelo OllamaClient do PhxClaw e imprime a resposta.
//! `cargo run -p phxclaw-ollama-adapter --example perguntar -- <modelo> "<pergunta>"`

use phxclaw_ollama_adapter::{OllamaClient, OllamaConfig, OllamaMessage};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let modelo = args.next().unwrap_or_else(|| "smollm2:135m".into());
    let pergunta = args.next().unwrap_or_else(|| "What is Rust?".into());
    let c = OllamaClient::new(OllamaConfig::default())?;
    println!("servidor Ollama {}", c.version().await?["version"]);
    println!("modelo  {modelo}\npergunta {pergunta}\n");
    let msgs = [OllamaMessage {
        role: "user".into(),
        content: pergunta,
        images: vec![],
    }];
    let limite = Some(json!({"temperature": 0, "num_predict": 60}));
    let r = c.chat(&modelo, &msgs, None, None, None, limite).await?;
    println!(
        "resposta: {}",
        r["message"]["content"].as_str().unwrap_or("").trim()
    );
    println!(
        "\n{} tokens gerados em {:.2} s",
        r["eval_count"],
        r["total_duration"].as_f64().unwrap_or(0.0) / 1e9
    );
    Ok(())
}
