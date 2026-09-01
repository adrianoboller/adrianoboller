//! Embute a proveniencia do build: commit, se a arvore estava suja, e o alvo.
//!
//! A auditoria externa da 0.18.0 pediu «bundle Git completo e commit
//! embutido», e o motivo e pratico: uma versao sem commit nao identifica build
//! nenhum. Dois pacotes com `0.18.0` no nome podem ser arvores diferentes, e
//! quem esta depurando em producao nao tem como saber qual esta rodando.
//!
//! Sem dependencia externa nenhuma -- e o `git` chamado aqui e opcional: fora
//! de um repositorio (num pacote de fontes extraido, por exemplo) o commit sai
//! vazio e o binario diz «desconhecido», que e a verdade.

use std::process::Command;

fn git(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|s| s.status.success())
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
        .unwrap_or_default()
}

fn main() {
    let commit = git(&["rev-parse", "HEAD"]);
    // Arvore suja e informacao de PRODUCAO, nao detalhe: um binario compilado
    // de arvore modificada nao corresponde ao commit que ele anuncia.
    let sujo = !git(&["status", "--porcelain"]).is_empty();

    println!("cargo:rustc-env=PHX_COMMIT={commit}");
    println!("cargo:rustc-env=PHX_SUJO={}", if sujo { "1" } else { "0" });
    println!(
        "cargo:rustc-env=PHX_ALVO={}",
        std::env::var("TARGET").unwrap_or_default()
    );

    // Recompilar quando o HEAD anda, para o commit embutido nao envelhecer --
    // que e o mesmo defeito do binario velho, num degrau mais alto.
    for p in [".git/HEAD", ".git/refs/heads"] {
        println!("cargo:rerun-if-changed=../../{p}");
    }
}
