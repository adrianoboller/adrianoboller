//! A guarda contra o pulo CALADO novo: varre os testes do workspace e reprova o lugar que
//! pula sem passar pelo [`crate::pulado`]. Catraca nasceu em 0 (SP000013, 01/10/2026), depois
//! da migracao dos 46 lugares que so imprimiam «PULADO».
//!
//! Duas formas de pulo calado, as duas vistas no repositorio antes da migracao:
//!
//! 1. texto com «pulado/pulada» numa string (`eprintln!("sem bwrap: pulado")`) sem um
//!    `pular(` ao lado -- o `eprintln!` de teste que passa e capturado, entao nem a linha
//!    aparece;
//! 2. `let Ok(..) = std::env::var(..) else { return }` (ou `if env::var(..).is_err() {
//!    return }`) sem `pular(` dentro do bloco -- o pulo que nem imprime.
//!
//! Por que em Rust, e nao so no `numeros.py` do dossie: o dossie CONTA (e publica a lista);
//! esta guarda REPROVA, na suite, no mesmo `cargo test` que o pulo tentaria enganar. Os dois
//! leem o mesmo criterio (string com `pulad[oa]`, linha sem `pular`), e o teste do crate
//! prova a regra nos dois sentidos com um pulo plantado numa copia.

use std::path::{Path, PathBuf};

/// Um lugar que pula calado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achado {
    /// Caminho relativo a raiz varrida.
    pub arquivo: String,
    pub linha: usize,
    /// `texto-de-pulo-sem-registro` ou `recurso-ausente-sem-registro`.
    pub regra: &'static str,
    pub trecho: String,
}

impl std::fmt::Display for Achado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{} [{}] {}",
            self.arquivo, self.linha, self.regra, self.trecho
        )
    }
}

/// O crate desta guarda: fica fora da varredura porque os proprios testes dele plantam o
/// texto que a regra reprova.
const PROPRIO: &str = "phxclaw-test-support";

/// Os arquivos de teste da raiz: `crates/*/tests/**/*.rs`, `apps/*/tests/**/*.rs` e os
/// unitarios `src/**/tests.rs` dos dois. A lista sai do disco, nunca de uma copia aqui.
pub fn arquivos_de_teste(raiz: &Path) -> Vec<PathBuf> {
    let mut saida = vec![];
    for grupo in ["crates", "apps"] {
        let Ok(crates) = std::fs::read_dir(raiz.join(grupo)) else {
            continue;
        };
        let mut crates: Vec<PathBuf> = crates.flatten().map(|e| e.path()).collect();
        crates.sort();
        for c in crates {
            if c.file_name().is_some_and(|n| n == PROPRIO) {
                continue;
            }
            let mut rs = vec![];
            andar(&c.join("tests"), &mut rs, &|p| {
                p.extension().is_some_and(|e| e == "rs")
            });
            andar(&c.join("src"), &mut rs, &|p| {
                p.file_name().is_some_and(|n| n == "tests.rs")
            });
            saida.extend(rs);
        }
    }
    saida.sort();
    saida
}

fn andar(dir: &Path, saida: &mut Vec<PathBuf>, quer: &dyn Fn(&Path) -> bool) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            andar(&p, saida, quer);
        } else if quer(&p) {
            saida.push(p);
        }
    }
}

/// Varre a raiz e devolve os lugares calados, em ordem de arquivo e linha.
pub fn pulos_calados(raiz: &Path) -> Vec<Achado> {
    let mut saida = vec![];
    for arq in arquivos_de_teste(raiz) {
        let Ok(texto) = std::fs::read_to_string(&arq) else {
            continue;
        };
        let rel = arq
            .strip_prefix(raiz)
            .unwrap_or(&arq)
            .to_string_lossy()
            .replace('\\', "/");
        saida.extend(pulos_calados_no_texto(&rel, &texto));
    }
    saida
}

/// «pulad» dentro de uma string literal da linha (qualquer caixa). Conta as aspas antes da
/// ocorrencia: impar e dentro da string. Linha de comentario nao conta.
fn texto_de_pulo(linha: &str) -> bool {
    let t = linha.trim_start();
    if t.starts_with("//") {
        return false;
    }
    let baixo = linha.to_ascii_lowercase();
    let mut de = 0;
    while let Some(i) = baixo[de..].find("pulad") {
        let pos = de + i;
        let aspas = linha[..pos].matches('"').count();
        if aspas % 2 == 1 {
            return true;
        }
        de = pos + 5;
    }
    false
}

/// A regra 2 so olha o que decide pelo ambiente: `let Ok/Some(..) = ..env::var(..) else {`
/// e `if ..env::var(..).is_err() {`.
fn abre_bloco_de_ambiente(linha: &str) -> bool {
    let t = linha.trim();
    if !t.contains("env::var") {
        return false;
    }
    (t.starts_with("let ") && t.ends_with("else {"))
        || (t.starts_with("if ") && t.contains(".is_err()") && t.ends_with('{'))
}

/// Os lugares calados de UM arquivo (o nome so entra no achado).
pub fn pulos_calados_no_texto(arquivo: &str, texto: &str) -> Vec<Achado> {
    let linhas: Vec<&str> = texto.lines().collect();
    let mut saida = vec![];
    for (i, lin) in linhas.iter().enumerate() {
        if texto_de_pulo(lin) {
            // «ao lado» = ate tres linhas para cada lado: o `pular(` com os argumentos
            // quebrados pelo rustfmt fica a uma ou duas linhas da string.
            let de = i.saturating_sub(3);
            let ate = (i + 4).min(linhas.len());
            if !linhas[de..ate].iter().any(|l| l.contains("pular(")) {
                saida.push(Achado {
                    arquivo: arquivo.into(),
                    linha: i + 1,
                    regra: "texto-de-pulo-sem-registro",
                    trecho: lin.trim().into(),
                });
            }
        }
        if abre_bloco_de_ambiente(lin) {
            // O bloco vai da chave que abre ate a que fecha no mesmo nivel.
            let mut nivel = 0i32;
            let mut fim = i;
            'fora: for (j, l) in linhas.iter().enumerate().skip(i) {
                for c in l.chars() {
                    match c {
                        '{' => nivel += 1,
                        '}' => {
                            nivel -= 1;
                            if nivel == 0 {
                                fim = j;
                                break 'fora;
                            }
                        }
                        _ => {}
                    }
                }
            }
            let bloco = &linhas[i..=fim];
            let volta = bloco.iter().any(|l| l.contains("return"));
            let registra = bloco.iter().any(|l| l.contains("pular("));
            if volta && !registra {
                saida.push(Achado {
                    arquivo: arquivo.into(),
                    linha: i + 1,
                    regra: "recurso-ausente-sem-registro",
                    trecho: lin.trim().into(),
                });
            }
        }
    }
    saida
}
