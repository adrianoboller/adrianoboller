//! O conferidor de **zero dependencias externas** -- a pétrea mais repetida
//! do `CLAUDE.md`, e a que o QA-PDCA achou sem guarda nenhuma.
//!
//! # O que faltava
//!
//! Os oito `Cargo.toml` de `crates/*/` so declaram `phxsql-*.workspace =
//! true` entre si hoje -- mas nada no repositorio IMPEDIA um `serde = "1.0"`
//! de entrar num proximo commit. `cargo build --offline` recusa por
//! ACIDENTE (falta a crate no cache local), nao por regra: numa maquina com
//! a crate ja em cache, ou com rede, o build passaria calado. Este modulo e
//! a regra escrita.
//!
//! # Por que olhar os NOMES do workspace, e nao o campo `source` do lock
//!
//! A primeira ideia foi procurar `source = "registry+..."` no `Cargo.lock`.
//! Ela tem um furo: uma dependencia de CAMINHO apontando para FORA do
//! workspace (`{ path = "../alguma-crate-de-fora" }`) tambem nao leva
//! `source` -- os oito pacotes do proprio workspace, que sao path deps entre
//! si, ja provam isso lendo o `Cargo.lock` de verdade (nenhum deles tem
//! `source`). Comparar os NOMES do `Cargo.lock` contra os nomes que o
//! `[workspace] members` do `Cargo.toml` raiz DECLARA fecha as duas portas
//! de uma vez: registro E caminho de fora.
//!
//! # Por que isto NAO se prova mutando o `Cargo.lock` ou o `Cargo.toml` de
//! verdade (e por que nao ha entrada deste defeito no catalogo de mutacao)
//!
//! Medido nesta rodada, dentro de uma copia isolada: acrescentar um pacote
//! FANTASMA ao `Cargo.lock` (sem nenhuma dependencia real apontando para
//! ele) nao sobrevive -- o proprio `cargo test` reescreve o arquivo e PODA a
//! entrada antes de qualquer teste rodar, porque nada no grafo resolvido a
//! referencia. E acrescentar uma dependencia de verdade (`serde = "1.0"`) a
//! um `Cargo.toml` quebra a resolucao ANTES de compilar qualquer coisa --
//! `cargo test --offline` falha com «no matching package named `serde`
//! found» pelo cargo, e nenhum teste Rust chega a rodar para reprovar nada.
//! As duas metades do mecanismo de `bancada/guardas/` (repor um trecho,
//! rodar so o binario de teste nomeado) pressupoem que o binario ainda
//! compila com o defeito reposto -- e aqui, por natureza, ele nao compila.
//!
//! Por isso a prova real deste modulo mora nos TESTES DESTE ARQUIVO, em duas
//! camadas: [`workspace_zero_dependencia_externa`] confere o `Cargo.lock` de
//! VERDADE (a garantia que protege o repositorio, hoje), e
//! `deteta_pacote_de_fora_do_workspace` confere a LOGICA de deteccao contra
//! um `Cargo.lock` de mentira embutido no proprio teste (a garantia que a
//! guarda entra no catalogo de mutacao de `bancada/guardas/` para provar:
//! mutar o FILTRO, nao o manifesto).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A raiz do workspace, a partir de onde este modulo esta compilando.
///
/// `crates/phxsql-server` -> `crates` -> raiz. `CARGO_MANIFEST_DIR` e
/// resolvido em tempo de COMPILACAO, entao isto nunca depende do diretorio
/// de onde o teste foi chamado.
pub fn raiz_do_workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/phxsql-server deveria ter dois diretorios acima")
        .to_path_buf()
}

/// Os nomes dos pacotes que o `[workspace] members` do `Cargo.toml` raiz
/// DECLARA -- lidos do proprio arquivo, e nao digitados aqui. Uma lista
/// digitada e a mesma armadilha que o `CLAUDE.md` ja nomeou para o KiB da
/// interface: quando um noveno membro entrar no workspace, a lista tem de
/// sair do codigo, nao ser lembrada por quem escreveu este modulo.
pub fn nomes_do_workspace(raiz: &Path) -> Result<BTreeSet<String>, String> {
    let cargo_toml = raiz.join("Cargo.toml");
    let texto = std::fs::read_to_string(&cargo_toml)
        .map_err(|e| format!("nao consegui ler {}: {e}", cargo_toml.display()))?;
    let membros = strings_entre_colchetes(&texto, "members");
    let mut nomes = BTreeSet::new();
    for membro in membros {
        let caminho = raiz.join(&membro).join("Cargo.toml");
        let texto_membro = std::fs::read_to_string(&caminho)
            .map_err(|e| format!("nao consegui ler {}: {e}", caminho.display()))?;
        let nome = nome_do_pacote(&texto_membro)
            .ok_or_else(|| format!("{} nao declara \"name = ...\"", caminho.display()))?;
        nomes.insert(nome);
    }
    Ok(nomes)
}

/// As strings entre `[` e `]` de um campo TOML no formato de lista, uma por
/// linha -- o formato que `members = [...]` usa neste repositorio. Nao e um
/// analisador de TOML geral: e o suficiente para o que este arquivo precisa
/// ler, e nada mais.
fn strings_entre_colchetes(texto: &str, campo: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut dentro = false;
    for linha in texto.lines() {
        let l = linha.trim();
        if !dentro {
            if l.starts_with(campo) && l.contains('[') {
                dentro = true;
            }
            continue;
        }
        if l.starts_with(']') {
            break;
        }
        if let Some(inicio) = l.find('"') {
            if let Some(fim) = l[inicio + 1..].find('"') {
                saida.push(l[inicio + 1..inicio + 1 + fim].to_string());
            }
        }
    }
    saida
}

/// O nome do pacote: a primeira linha `name = "..."` do arquivo. Funciona
/// porque `[package]` e sempre a primeira secao de um `Cargo.toml` de crate
/// nesta casa -- um `[[bin]]` com o seu proprio `name = "..."` so aparece
/// DEPOIS, e a primeira ocorrencia ja resolveu antes de chegar la.
fn nome_do_pacote(texto: &str) -> Option<String> {
    for linha in texto.lines() {
        let l = linha.trim();
        if let Some(v) = l.strip_prefix("name = \"") {
            if let Some(fim) = v.find('"') {
                return Some(v[..fim].to_string());
            }
        }
    }
    None
}

/// Os pacotes que um `Cargo.lock` declara, como `(nome, versao)` -- so os
/// dois primeiros campos de cada bloco `[[package]]`, que e tudo que este
/// conferidor precisa.
pub fn pacotes_do_lock(texto: &str) -> Vec<(String, String)> {
    let mut pacotes = Vec::new();
    let mut dentro_de_pacote = false;
    let mut nome_atual: Option<String> = None;
    for linha in texto.lines() {
        let l = linha.trim();
        if l == "[[package]]" {
            dentro_de_pacote = true;
            nome_atual = None;
            continue;
        }
        if !dentro_de_pacote {
            continue;
        }
        if let Some(v) = l.strip_prefix("name = \"") {
            if let Some(fim) = v.find('"') {
                nome_atual = Some(v[..fim].to_string());
            }
            continue;
        }
        if let Some(v) = l.strip_prefix("version = \"") {
            if let Some(fim) = v.find('"') {
                if let Some(nome) = nome_atual.take() {
                    pacotes.push((nome, v[..fim].to_string()));
                }
            }
            // `name` sempre vem imediatamente antes de `version` no formato
            // do Cargo.lock (v3/v4) -- o bloco ja deu o que interessa aqui.
            dentro_de_pacote = false;
        }
    }
    pacotes
}

/// Os pacotes de `pacotes` cujo nome NAO esta em `permitidos`.
///
/// So a MENTE do defeito: comparacao de conjuntos, sem tocar disco nem
/// cargo. E por isso que da para provar por mutacao (`bancada/guardas/`) --
/// mutar esta funcao nao mexe em manifesto nenhum.
pub fn dependencias_externas(
    pacotes: &[(String, String)],
    permitidos: &BTreeSet<String>,
) -> Vec<(String, String)> {
    pacotes
        .iter()
        .filter(|(n, _)| !permitidos.contains(n))
        .cloned()
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// **A guarda de verdade.** Le o `Cargo.lock` e o `Cargo.toml` REAIS do
    /// workspace e confere que todo pacote resolvido e um dos membros
    /// declarados -- nem um a mais. E o teste que reprova no dia em que
    /// alguem acrescentar uma dependencia de fora, com rede ou sem: ao
    /// contrario de `cargo build --offline`, ele nao depende do cache local
    /// nem da conectividade estarem numa condicao especifica.
    #[test]
    fn workspace_zero_dependencia_externa() {
        let raiz = raiz_do_workspace();
        let permitidos = nomes_do_workspace(&raiz).expect("workspace legivel");
        assert_eq!(
            permitidos.len(),
            8,
            "o workspace tem {} membro(s) declarado(s), nao 8 -- se um crate \
             novo entrou de proposito, o numero aqui e so uma conferencia \
             de sanidade e pode subir junto",
            permitidos.len()
        );

        let cargo_lock = raiz.join("Cargo.lock");
        let texto = std::fs::read_to_string(&cargo_lock).expect("Cargo.lock existe");
        let pacotes = pacotes_do_lock(&texto);
        assert!(
            pacotes.len() >= permitidos.len(),
            "o Cargo.lock tem menos pacotes ({}) que membros do workspace \
             ({}) -- o analisador deste modulo esta lendo errado",
            pacotes.len(),
            permitidos.len()
        );

        let externos = dependencias_externas(&pacotes, &permitidos);
        assert!(
            externos.is_empty(),
            "dependencia de fora do workspace no Cargo.lock: {externos:?} -- \
             zero dependencias externas e regra petrea do CLAUDE.md (\"so a \
             std e o proprio workspace\"); se isto reprovou, alguem \
             acrescentou uma crate de fora"
        );
    }

    /// **A guarda da LOGICA**, contra um `Cargo.lock` de mentira -- e a que
    /// entra no catalogo de mutacao de `bancada/guardas/`, porque mutar o
    /// filtro de [`dependencias_externas`] nao mexe em manifesto nenhum.
    ///
    /// Prova real: troque o corpo de `dependencias_externas` para sempre
    /// devolver `Vec::new()` (a guarda desligada, medindo e nunca
    /// reprovando) e este teste cai -- o `serde` de mentira abaixo passaria
    /// batido.
    #[test]
    fn deteta_pacote_de_fora_do_workspace() {
        let lock_de_mentira = "\
[[package]]
name = \"phxsql-core\"
version = \"0.18.0\"

[[package]]
name = \"serde\"
version = \"1.0.219\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
dependencies = [
 \"serde_core\",
]
";
        let mut permitidos = BTreeSet::new();
        permitidos.insert("phxsql-core".to_string());

        let pacotes = pacotes_do_lock(lock_de_mentira);
        assert_eq!(
            pacotes,
            vec![
                ("phxsql-core".to_string(), "0.18.0".to_string()),
                ("serde".to_string(), "1.0.219".to_string()),
            ],
            "o analisador do Cargo.lock nao leu os dois pacotes certos"
        );

        let externos = dependencias_externas(&pacotes, &permitidos);
        assert_eq!(
            externos,
            vec![("serde".to_string(), "1.0.219".to_string())],
            "o serde de mentira nao foi apontado como externo"
        );
    }

    /// O caminho so: um `Cargo.lock` onde tudo esta na lista permitida nao
    /// acusa nada -- sem isto, uma guarda quebrada que sempre devolve "tem
    /// externo" passaria despercebida (e reprovaria todo `cargo test`).
    #[test]
    fn sem_pacote_de_fora_nao_acusa_nada() {
        let lock_de_mentira = "\
[[package]]
name = \"phxsql-core\"
version = \"0.18.0\"

[[package]]
name = \"phxsql-store\"
version = \"0.18.0\"
dependencies = [
 \"phxsql-core\",
]
";
        let mut permitidos = BTreeSet::new();
        permitidos.insert("phxsql-core".to_string());
        permitidos.insert("phxsql-store".to_string());

        let pacotes = pacotes_do_lock(lock_de_mentira);
        let externos = dependencias_externas(&pacotes, &permitidos);
        assert!(externos.is_empty(), "{externos:?}");
    }

    /// O `Cargo.toml` raiz declara exatamente os oito membros que os testes
    /// acima assumem -- se um crate for acrescentado ou removido do
    /// workspace, este teste diz isso antes de qualquer coisa parecer
    /// misterioso nos de cima.
    #[test]
    fn os_nomes_do_workspace_batem_com_os_diretorios_de_crates() {
        let raiz = raiz_do_workspace();
        let permitidos = nomes_do_workspace(&raiz).expect("workspace legivel");
        let mut esperados = BTreeSet::new();
        for entrada in std::fs::read_dir(raiz.join("crates")).expect("crates/ existe") {
            let entrada = entrada.expect("entrada legivel");
            if entrada.path().join("Cargo.toml").is_file() {
                let nome = nome_do_pacote(
                    &std::fs::read_to_string(entrada.path().join("Cargo.toml")).unwrap(),
                )
                .unwrap();
                esperados.insert(nome);
            }
        }
        assert_eq!(
            permitidos, esperados,
            "os membros declarados em Cargo.toml e os diretorios em crates/ divergem"
        );
    }
}
