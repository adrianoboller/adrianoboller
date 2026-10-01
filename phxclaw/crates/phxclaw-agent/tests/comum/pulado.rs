//! O pulo VISIVEL de um teste que depende de recurso da maquina (binario, sandbox, modelo).
//!
//! Por que existe: o libtest nao tem «ignorado em tempo de execucao». O teste que faz
//! `eprintln!("PULADO"); return;` sai `ok` e entra no placar como PASSOU -- e o `eprintln!`
//! de teste que passa e CAPTURADO, entao nem a linha aparece sem `--nocapture`. Medido em
//! 01/10/2026 (prova F): `segredos` com `PHXCLAW_GITLEAKS_BIN=/nao/existe` dava
//! `test result: ok. 3 passed`, igual a corrida que varreu de verdade.
//!
//! O que faz, sem quebrar quem nao tem o recurso: registra o pulo numa linha JSON em
//! `target/tmp/pulados.jsonl` (`CARGO_TARGET_TMPDIR`) -- crate, teste, recurso, motivo -- e
//! o teste continua saindo `ok`. Quem DECIDE e o portao, nao o teste: ele apaga o arquivo
//! antes da suite, conta as linhas depois e publica «N passam, dos quais P pulados»; na
//! maquina do integrador, que tem os binarios, pulo de recurso exigido e NoGo. A decisao
//! fica fora do teste de proposito: uma variavel `PHXCLAW_*` para «exigir» teria de entrar
//! no catalogo do `config.json` (a catraca do `tools/config_catalogo.py` reprova nome solto).
//!
//! Uso: `#[path = "comum/pulado.rs"] mod pulado;` e `pulado::pular("gitleaks", "motivo")`
//! antes do `return`.

#![allow(dead_code)]

use std::io::Write;
use std::path::PathBuf;

/// O registro de pulos desta arvore de compilacao.
pub fn registro() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pulados.jsonl")
}

/// Registra o pulo e volta (o chamador faz o `return`). O nome do teste e o da thread: o
/// libtest roda cada teste numa thread com o nome dele, e o `#[tokio::test]` padrao roda o
/// corpo nela.
pub fn pular(recurso: &str, motivo: &str) {
    let teste = std::thread::current()
        .name()
        .unwrap_or("(sem nome)")
        .to_string();
    let linha = serde_json::json!({
        "crate": env!("CARGO_PKG_NAME"),
        "teste": teste,
        "recurso": recurso,
        "motivo": motivo,
    })
    .to_string();
    let arq = registro();
    // UMA escrita com O_APPEND, a linha ja com o `\n`: o `writeln!` num `File` faz uma
    // chamada por pedaco, e na primeira medicao (4 testes em paralelo) duas linhas sairam
    // coladas e uma vazia. Pulo que nao se registra e o verde calado de novo: ai falha.
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&arq)
        .and_then(|mut f| f.write_all(format!("{linha}\n").as_bytes()))
        .unwrap_or_else(|e| panic!("pulo de {teste} nao registrado em {}: {e}", arq.display()));
    eprintln!(
        "PULADO ({recurso}): {teste}: {motivo} -- registrado em {}",
        arq.display()
    );
}
