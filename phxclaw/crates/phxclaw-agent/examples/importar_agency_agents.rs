//! Importa os papeis do agency-agents para `config/agents` (ver `importar_papeis`).
//!
//! cargo run -p phxclaw-agent --example importar_agency_agents -- REPO COMMIT [PASTA]
//!
//! `REPO` e o clone do agency-agents; `COMMIT` e o commit que se quer importar, e o clone
//! tem de estar EXATAMENTE nele e limpo -- importar de uma arvore que nao e o commit
//! declarado gravaria nos manifestos uma origem que nao e a verdadeira. `PASTA` e a dos
//! manifestos (padrao: a `config/agents` deste repositorio). Depois, regere a interface:
//! `cargo run -p phxclaw-agent --example equipe_json`.

use phxclaw_agent::importar_papeis;
use std::path::PathBuf;
use std::process::Command;

fn git(repo: &PathBuf, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let uso = "uso: importar_agency_agents REPO COMMIT [PASTA]";
    let repo = PathBuf::from(args.next().ok_or(uso)?);
    let commit = args.next().ok_or(uso)?;
    let pasta = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/agents"));
    let head = git(&repo, &["rev-parse", "HEAD"])?;
    let pedido = git(&repo, &["rev-parse", &format!("{commit}^{{commit}}")])?;
    if head != pedido {
        return Err(format!(
            "o clone esta em {head}, nao em {pedido}: faca checkout do commit pedido"
        ));
    }
    let sujo = git(&repo, &["status", "--porcelain"])?;
    if !sujo.is_empty() {
        return Err(format!(
            "o clone tem mudancas fora do commit:\n{sujo}\nimporte de uma arvore limpa"
        ));
    }
    let r = importar_papeis::importar(&repo, &head, &pasta)?;
    println!(
        "{} achados, {} importados, {} recusados, {} removidos de importacao anterior ({} @ {})",
        r.achados,
        r.importados,
        r.recusados.len(),
        r.removidos,
        r.origem,
        &r.commit[..r.commit.len().min(12)]
    );
    for x in &r.recusados {
        println!("RECUSADO {}: {}", x.arquivo, x.motivo);
    }
    // O que entrou sem shell ou rede tem de aparecer: calado, o operador acharia que o
    // papel roda com as ferramentas do `tools:` de origem.
    println!(
        "{} papeis pediram shell ou rede e entraram sem (concessao do operador: {})",
        r.pedidos_de_concessao.len(),
        phxclaw_agent::equipe::ARQUIVO_CONCESSOES
    );
    for a in &r.avisos {
        println!("aviso: {a}");
    }
    Ok(())
}
