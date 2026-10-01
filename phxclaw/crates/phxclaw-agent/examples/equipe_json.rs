//! Gera `apps/phxclaw-ui/assets/equipe.json` do catalogo de `config/agents` (ou de
//! `PHXCLAW_AGENTES_DIR`). Nenhum numero do arquivo e digitado: a contagem, os grupos e as
//! fichas saem de `Equipe::json_da_interface`, a mesma funcao que o teste confere.
//!
//! cargo run -p phxclaw-agent --example equipe_json [-- SAIDA]

use phxclaw_agent::equipe::{ARQUIVO_DA_INTERFACE, Equipe};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let equipe = Equipe::do_ambiente()?;
    let saida = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(ARQUIVO_DA_INTERFACE)
        });
    std::fs::write(&saida, equipe.arquivo_da_interface()).map_err(|e| e.to_string())?;
    println!(
        "{} papeis de {} gravados em {}",
        equipe.len(),
        equipe.pasta.display(),
        saida.display()
    );
    Ok(())
}
