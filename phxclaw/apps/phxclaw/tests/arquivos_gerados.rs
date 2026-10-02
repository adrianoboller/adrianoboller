//! Arquivo gerado que envelhece calado: `apps/phxclaw-ui/assets/ferramentas.json` e
//! `docs/AGENTE_AUTONOMO.md` saem do binario (`phxclaw ferramentas`, `--help`, `equipe
//! listar`), e fonte mudado sem regerar reprova aqui -- igual ao que o `equipe.json` ja tem
//! em `crates/phxclaw-agent/tests/equipe.rs`.
//!
//! Os dois variam por maquina (sem bwrap nao ha shell, sem Chromium nao ha navegador). O
//! arquivo versionado e o da maquina do integrador, com os dois; aqui, se faltar um deles, a
//! diferenca nao prova arquivo velho, e o teste PULA registrando (nunca calado).

use phxclaw_test_support::pulado;
use std::path::{Path, PathBuf};
use std::process::Command;

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// O binario que o cargo acabou de construir, com o ambiente que o gerador usa: sem
/// `PHXCLAW_CAPACIDADES`, para a coluna «concedida» ser a do padrao.
fn phxclaw() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_phxclaw"));
    c.env_remove("PHXCLAW_CAPACIDADES").current_dir(raiz());
    c
}

/// O que falta nesta maquina para a montagem ser a do arquivo versionado.
fn recurso_que_falta() -> Option<&'static str> {
    if phxclaw_agent::arquivos::achar_bwrap().is_none() {
        return Some("bwrap");
    }
    if phxclaw_browser::find_chromium().is_none() {
        return Some("chromium");
    }
    None
}

#[test]
fn ferramentas_json_da_interface_esta_em_dia() {
    let o = phxclaw().arg("ferramentas").output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let gerado = String::from_utf8(o.stdout).unwrap();
    let arquivo = raiz().join("apps/phxclaw-ui/assets/ferramentas.json");
    let atual =
        std::fs::read_to_string(&arquivo).expect("falta o arquivo: tools/gerar_assets_ui.sh");
    // `println!` acrescenta o `\n`; o `cp` do gerador guarda exatamente isso.
    if atual == gerado {
        return;
    }
    if let Some(r) = recurso_que_falta() {
        pulado::pular(r, "a montagem desta maquina nao e a do arquivo versionado");
        return;
    }
    let a: serde_json::Value = serde_json::from_str(&atual).unwrap();
    let g: serde_json::Value = serde_json::from_str(&gerado).unwrap();
    let nomes = |v: &serde_json::Value| -> Vec<String> {
        v["ferramentas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["nome"].as_str().unwrap().to_string())
            .collect()
    };
    panic!(
        "ferramentas.json velho: rode tools/gerar_assets_ui.sh (arquivo {} ferramentas, \
         binario {}; so no arquivo {:?}; so no binario {:?})",
        a["total"],
        g["total"],
        nomes(&a)
            .iter()
            .filter(|n| !nomes(&g).contains(n))
            .collect::<Vec<_>>(),
        nomes(&g)
            .iter()
            .filter(|n| !nomes(&a).contains(n))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn agente_autonomo_md_esta_em_dia() {
    // O mesmo gerador, em modo de conferencia: regera em memoria e compara sem as datas.
    let o = Command::new("python3")
        .args([
            "tools/gerar_doc_agente.py",
            "--conferir",
            env!("CARGO_BIN_EXE_phxclaw"),
        ])
        .env_remove("PHXCLAW_CAPACIDADES")
        .current_dir(raiz())
        .output()
        .unwrap();
    let saida = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    if o.status.success() {
        return;
    }
    if o.status.code() == Some(3)
        && let Some(r) = recurso_que_falta()
    {
        pulado::pular(
            r,
            "a montagem desta maquina nao e a do documento versionado",
        );
        return;
    }
    panic!(
        "docs/AGENTE_AUTONOMO.md velho ou gerador com erro ({:?}):\n{saida}",
        o.status.code()
    );
}
