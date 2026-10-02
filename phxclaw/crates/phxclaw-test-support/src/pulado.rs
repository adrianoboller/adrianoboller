//! O pulo VISIVEL de um teste que depende de recurso da maquina (binario, sandbox, modelo).
//!
//! Por que existe: o libtest nao tem «ignorado em tempo de execucao». O teste que faz
//! `eprintln!("PULADO"); return;` sai `ok` e entra no placar como PASSOU -- e o `eprintln!`
//! de teste que passa e CAPTURADO, entao nem a linha aparece sem `--nocapture`. Medido em
//! 01/10/2026 (prova F): `segredos` com `PHXCLAW_GITLEAKS_BIN=/nao/existe` dava
//! `test result: ok. 3 passed`, igual a corrida que varreu de verdade.
//!
//! O que faz, sem quebrar quem nao tem o recurso: registra o pulo numa linha JSON em
//! `target/tmp/pulados.jsonl` -- crate, teste, recurso, motivo -- e o teste continua saindo
//! `ok`. Quem DECIDE e o portao (`tools/suite.sh`), nao o teste: ele apaga o arquivo antes
//! da suite, conta as linhas depois e publica «N passam, P pulados, F falham»; na maquina do
//! integrador, que tem os binarios, pulo de recurso que a maquina TEM e NoGo. A decisao fica
//! fora do teste de proposito: uma variavel `PHXCLAW_*` para «exigir» teria de entrar no
//! catalogo do `config.json` (a catraca do `tools/config_catalogo.py` reprova nome solto).
//!
//! Por que mora num crate e nao num `#[path = "comum/pulado.rs"]`: sete crates pulam, e
//! sete copias do arquivo seriam a mesma decisao escrita sete vezes. `use
//! phxclaw_test_support::pulado;` e `pulado::pular("bwrap", "motivo")` antes do `return`.
//!
//! O `recurso` e o NOME que o portao confere na maquina: binario (`bwrap`, `openssl`,
//! `chromium`, `soffice`...), variavel (`PHXCLAW_PROVA_VISAO`) ou caminho. Nome que o portao
//! nao sabe conferir conta como pulo, nunca como NoGo -- por isso ele e curto e literal.
//! O `motivo` nao leva a palavra «pulado»: ela ja sai daqui, e o contador do dossie
//! (`tools/dossie/numeros.py`) le qualquer string com ela como pulo calado.

use std::io::Write;
use std::path::PathBuf;

/// O registro de pulos desta arvore de compilacao: `<target>/tmp/pulados.jsonl`.
///
/// `CARGO_TARGET_TMPDIR` so existe em tempo de COMPILACAO dos testes de integracao; nos
/// unitarios de `src/**/tests.rs` nao existe, e aqui dentro (crate compilado como
/// dependencia) tambem nao. O que TODO teste tem e o proprio executavel, em
/// `<target>/<perfil>/deps/NOME-HASH`: tres niveis acima e o `target`, inclusive com
/// `CARGO_TARGET_DIR` apontando para fora da arvore. Sem executavel legivel, cai no
/// `target` do workspace (`CARGO_MANIFEST_DIR/../../target`), que e o mesmo lugar quando
/// ninguem mudou o `CARGO_TARGET_DIR`.
pub fn registro() -> PathBuf {
    let target = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
        });
    target.join("tmp").join("pulados.jsonl")
}

/// O crate do teste que esta rodando. O cargo poe `CARGO_PKG_NAME` no ambiente do
/// `cargo test`; rodando o binario a mao, sai do nome dele (`NOME-HASH`).
fn crate_do_teste() -> String {
    if let Ok(n) = std::env::var("CARGO_PKG_NAME") {
        return n;
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .map(|s| s.rsplit_once('-').map(|(a, _)| a.to_string()).unwrap_or(s))
        .unwrap_or_else(|| "(sem crate)".into())
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
        "crate": crate_do_teste(),
        "teste": teste,
        "recurso": recurso,
        "motivo": motivo,
    })
    .to_string();
    let arq = registro();
    if let Some(p) = arq.parent() {
        let _ = std::fs::create_dir_all(p);
    }
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
