//! Cria uma tarefa no `phxclaw servir`, espera e mostra a resposta e os artefatos.
//!
//! ```text
//! PHXCLAW_API_TOKEN=... cargo run -p phxclaw-sdk --example tarefa -- "resuma o README.md"
//! ```
//!
//! `PHXCLAW_URL` muda o servidor (padrao `http://127.0.0.1:8787`) e `PHXCLAW_MODELO` o
//! modelo (padrao: o do servidor).

use phxclaw_sdk::{Cliente, NovaTarefa, TaskStatus};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let objetivo = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if objetivo.trim().is_empty() {
        eprintln!("uso: tarefa <objetivo>");
        std::process::exit(64);
    }
    let url = std::env::var("PHXCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
    let c = Cliente::do_ambiente(&url)?;
    let id = c
        .criar(&NovaTarefa {
            objective: objetivo,
            model: std::env::var("PHXCLAW_MODELO").ok(),
            ..NovaTarefa::default()
        })
        .await?;
    println!("tarefa {id}");
    let t = c.aguardar(&id, Duration::from_secs(600)).await?;
    for p in &t.steps {
        println!("  {:>2} {:<10} {}", p.n, p.outcome, p.summary);
    }
    println!("estado: {:?}", t.status);
    if let Some(r) = &t.answer {
        println!("resposta: {r}");
    }
    for a in &t.artifacts {
        println!("artefato: {} ({} bytes)", a.path, a.bytes);
    }
    if t.status != TaskStatus::Completed {
        std::process::exit(2);
    }
    Ok(())
}
