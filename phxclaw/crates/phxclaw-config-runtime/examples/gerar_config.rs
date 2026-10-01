//! Regrava os artefatos gerados do catalogo do `config.json` (schema, exemplo e o
//! catalogo da tela). O teste `artefatos_gerados_estao_em_dia` manda rodar isto.

use phxclaw_config_runtime::agente::gerar::artefatos;
use std::path::Path;

fn main() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (rel, conteudo) in artefatos() {
        let p = raiz.join(rel);
        std::fs::write(&p, conteudo).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        println!("gravado: {rel}");
    }
}
