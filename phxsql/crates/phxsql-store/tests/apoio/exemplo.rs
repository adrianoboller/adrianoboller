//! O binario de um EXEMPLO medidor, ao lado do binario do teste que o roda --
//! e a recusa de medir com binario velho.
//!
//! Um lugar so para as provas que rodam um exemplo e cobram o que ele
//! reportou (`catraca-fsync-por-fecho`, `catraca-fsync-da-subida`,
//! `fecho-em-paralelo-conta-os-mesmos-fsync`). Cada uma tinha a sua copia do
//! `caminho_do_exemplo`, e a segunda catraca medida por exemplo (pedido 533)
//! faria a terceira copia do `mais_novo_que_o_exemplo` -- a regra de «binario
//! velho mede o passado» escrita tres vezes, e a que alguem esquecesse de
//! corrigir seria a que mede o passado. Entra por `#[path]`, como o
//! `examples/apoio/strace.rs`: sub-diretorio de `tests/` sem `main.rs` nao e
//! alvo de teste.
//!
//! `dead_code` fica permitido pelo mesmo motivo do `comum/mod.rs`: cada
//! arquivo de `tests/` e um binario separado, e quem usa so o caminho deixa
//! as outras duas sem uso.
#![allow(dead_code)]

/// O binario do exemplo `nome`, ao lado do binario deste teste.
///
/// `cargo test` compila os exemplos junto, entao ele existe -- e procura-lo
/// pelo `current_exe()` e' o que faz o teste medir o MESMO perfil em que ele
/// proprio roda. Chamar `cargo run` daqui seria cargo dentro de cargo, com a
/// trava de build do cargo de fora ainda na mao.
pub fn caminho_do_exemplo(nome: &str) -> Option<std::path::PathBuf> {
    let eu = std::env::current_exe().ok()?;
    let deps = eu.parent()?; // target/<perfil>/deps
    for base in [deps.parent(), Some(deps)].into_iter().flatten() {
        let c = base.join("examples").join(nome);
        if c.exists() {
            return Some(c);
        }
    }
    None
}

/// Recusa medir com binario VELHO.
///
/// `cargo test -p phxsql-store --test catraca-fsync-por-fecho` compila o
/// teste e **nao** compila os exemplos: o binario do medidor fica o da rodada
/// passada, e a catraca publica o numero de ontem. Foi assim que uma rodada
/// inteira de ganhos ficou invisivel na bancada desta casa -- `cargo build
/// --release` nao recompila example, e o medidor media o passado.
///
/// O crivo e' o mesmo que a lei descreve: fonte mais novo que o binario que o
/// mede. Comparar com o binario DESTE teste nao serviria -- mexer so' neste
/// arquivo o deixa mais novo que um exemplo que continua em dia.
///
/// O alcance e' `src/` inteiro (o exemplo linka o crate), **so' o `.rs` (ou
/// `.../main.rs`) deste exemplo** -- nao `examples/` inteiro --, e os
/// MODULOS COMPARTILHADOS de `examples/`. Exemplos compilam independentes um
/// do outro: `examples/` largo reprovava esta catraca por edicao num exemplo
/// ALHEIO (pedido 409), e ja levou uma frente a diagnosticar "e' ambiental"
/// quando o defeito era o alcance da guarda.
///
/// O segundo defeito (achado na integracao do 409, mesmo dia): o alcance
/// estreito perdeu `examples/apoio/`, que **nao e' exemplo nenhum** -- e'
/// codigo que `custo-do-excluir.rs` e `fsync-por-operacao.rs` incluem por
/// `#[path = "apoio/strace.rs"]`. A distincao vem da propria convencao do
/// Cargo, nao de `"apoio"` escrito aqui (lista que envelheceria calada):
///
/// * arquivo solto em `examples/*.rs` -> exemplo independente -> so' o nosso;
/// * sub-diretorio de `examples/` SEM `main.rs` -> nao e' exemplo, e'
///   modulo compartilhado que qualquer exemplo pode incluir -> entra
///   INTEIRO na pilha, para qualquer exemplo que esta funcao meca;
/// * sub-diretorio COM `main.rs` -> exemplo em forma de pasta (convencao do
///   Cargo para `examples/<nome>/main.rs`) -> so' entra se for o nosso.
pub fn mais_novo_que_o_exemplo(exemplo: &std::path::Path) -> Vec<String> {
    let Ok(bin) = exemplo.metadata().and_then(|m| m.modified()) else {
        return Vec::new();
    };
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir_examples = raiz.join("examples");
    let nome = exemplo.file_stem();
    // O proprio exemplo: arquivo solto `examples/<nome>.rs` OU pasta
    // `examples/<nome>/main.rs` -- so' um dos dois existe de cada vez.
    let proprios = [
        nome.map(|n| dir_examples.join(n).with_extension("rs")),
        nome.map(|n| dir_examples.join(n).join("main.rs")),
    ];
    let mut novos = Vec::new();
    let mut pilha = vec![raiz.join("src")];
    for p in proprios.into_iter().flatten() {
        if p.metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| t > bin)
        {
            novos.push(p.display().to_string());
        }
    }
    // Modulos compartilhados: sub-diretorio de `examples/` sem `main.rs`.
    if let Ok(itens) = std::fs::read_dir(&dir_examples) {
        for item in itens.flatten() {
            let c = item.path();
            if c.is_dir() && !c.join("main.rs").exists() {
                pilha.push(c);
            }
        }
    }
    while let Some(dir) = pilha.pop() {
        let Ok(itens) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in itens.flatten() {
            let c = item.path();
            if c.is_dir() {
                pilha.push(c);
                continue;
            }
            if c.extension().is_some_and(|e| e == "rs")
                && item
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t > bin)
            {
                novos.push(c.display().to_string());
            }
        }
    }
    novos.sort();
    novos
}

/// Le a linha `catraca:` do exemplo e devolve os campos.
pub fn campos(saida: &str) -> Option<std::collections::HashMap<String, String>> {
    let linha = saida.lines().find(|l| l.starts_with("catraca:"))?;
    Some(
        linha["catraca:".len()..]
            .split(';')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect(),
    )
}
