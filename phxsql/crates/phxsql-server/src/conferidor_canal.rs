//! O conferidor do CANAL: quem le linha de soquete fora do motor.
//!
//! # Por que ele existe
//!
//! Pedido 439. A petrea «funcao e comando vem do mesmo motor» tem um motor
//! para a pergunta «quanto eu reservo numa linha que vem do soquete»: o
//! `phxsql_core::fio::Canal`, com o `ler_ate` e o teto dito em voz alta. O
//! pedido 434 levou para ele cinco leituras que moravam fora; o QA achou a
//! sexta depois (o cliente SMTP, `email.rs`), com teto de TEMPO e nenhum de
//! TAMANHO -- medido, 8.388.610 bytes tirados do soquete numa linha so. Cada
//! uma foi achada por varredura a olho, e a proxima seria achada do mesmo
//! jeito, ou nao seria. Esta catraca e o que troca «alguem vai olhar» por «o
//! teste reprova».
//!
//! # O que ele conta
//!
//! Toda chamada a `read_line` e `read_until` em `crates/*/src`, menos as
//! catalogadas em [`ISENTOS`] -- por ARQUIVO e com a quantidade esperada,
//! como o `conferidor_temporarios`: uma leitura nova num arquivo ja isento
//! tambem reprova, porque o motivo escrito ao lado vale para as que estao
//! listadas, e nao para as que alguem acrescentar amanha.
//!
//! # Por que por NOME de metodo, e nao «leitura de soquete»
//!
//! Porque distinguir soquete de arquivo exigiria saber o TIPO do leitor, e
//! isso o texto nao diz. A proposta do QA
//! (`docs/propostas/qa-inventario-mesmo-motor-2026-09-23.md` §3) casava o
//! metodo num raio de um nome com cara de soquete -- e o raio erra nos dois
//! sentidos. Contar todo `read_line`/`read_until` e isentar por nome, com o
//! motivo, e barulhento no primeiro dia (14 sitios, todos catalogados) e
//! exato dali em diante: quem acrescenta um le a recusa e decide.
//!
//! # O que ele NAO pega -- dizer e parte da guarda
//!
//! * `.lines()`, `read_to_end` e `read_to_string` ficam fora: aqui eles leem
//!   arquivo e texto na memoria centenas de vezes, e um soquete lido assim
//!   passaria. Nenhum le soquete hoje (conferido por `grep` em 24/09/2026).
//! * Um laco manual com `.bytes()` ou `fill_buf`/`consume` monta a mesma
//!   linha sem nenhum dos dois nomes. O `descartar_ate_a_quebra` do servidor
//!   usa `fill_buf` DE PROPOSITO -- drena sem guardar, outra pergunta.
//! * `crates/*/tests` fica fora: ali moram os CLIENTES de teste, que leem a
//!   resposta do servidor com `read_line` porque sao o outro lado do fio.
//! * Um `Canal` usado ERRADO (`ler_ate` com teto absurdo) nao e ausencia de
//!   `Canal`; tem prova propria no `fio.rs`.
//!
//! ```bash
//! cargo test -p phxsql-server --lib conferidor_canal
//! ```

use std::path::{Path, PathBuf};

/// Os metodos procurados, montados em duas metades DE PROPOSITO: escritos
/// inteiros, estas linhas casariam consigo mesmas.
const PADROES: [&str; 2] = [concat!(".read_", "line("), concat!(".read_", "until(")];

/// A raiz do repositorio, a partir do `Cargo.toml` deste crate.
fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("o crate mora em <raiz>/crates/phxsql-server")
        .to_path_buf()
}

/// Arquivos que podem ler linha fora do `Canal`, com a quantidade esperada e
/// o motivo. Caminho relativo a raiz do repositorio.
pub const ISENTOS: &[(&str, usize, &str)] = &[
    (
        "crates/phxsql-core/src/fio.rs",
        2,
        "e o proprio motor: o `Canal::ler_decidindo` le a linha, com o teto, \
         nas duas partes da leitura decidida (pedido 442)",
    ),
    (
        "crates/phxsql-cmd/src/main.rs",
        1,
        "le o TECLADO (`stdin`), e nao um soquete: do outro lado esta o \
         proprio operador",
    ),
    (
        "crates/phxsql-odbc/src/conexao.rs",
        4,
        "os servidores de MENTIRA dos testes do driver, dentro de \
         `#[cfg(test)] mod testes`",
    ),
    (
        "crates/phxsql-odbc/src/lib.rs",
        1,
        "servidor de mentira de teste, dentro de `#[cfg(test)] mod testes`",
    ),
    (
        "crates/phxsql-server/src/apoio_teste.rs",
        2,
        "o rele SMTP falso dos testes (`rele_falso`): e o SERVIDOR de mentira \
         lendo o comando do cliente",
    ),
    (
        "crates/phxsql-server/src/servidor.rs",
        4,
        "clientes de teste dentro de `testes_firewall_e_mensagens`, \
         `testes_das_threads` e `testes_da_saude_do_disco` -- o outro lado do \
         fio, lendo a resposta",
    ),
];

/// Uma leitura de linha fora do catalogo.
#[derive(Debug, Clone)]
pub struct Solta {
    pub arquivo: String,
    pub linha: usize,
    pub texto: String,
}

/// O que a varredura achou.
#[derive(Debug, Default)]
pub struct Relatorio {
    /// As leituras que contam para a catraca.
    pub soltas: Vec<Solta>,
    /// Arquivos isentos cuja quantidade mudou: `(arquivo, esperado, achado)`.
    /// Vale nos DOIS sentidos, pelo mesmo motivo do `conferidor_temporarios`:
    /// catalogo que sobra e catalogo que ninguem le.
    pub isentos_mudaram: Vec<(String, usize, usize)>,
}

/// Quantas leituras de linha esta linha de fonte faz. Comentario nao le nada:
/// sem este corte, a documentacao que explica o defeito seria o defeito.
fn leituras_na_linha(linha: &str) -> usize {
    if linha.trim_start().starts_with("//") {
        return 0;
    }
    PADROES.iter().map(|p| linha.matches(p).count()).sum()
}

/// Anda `crates/*/src` do repositorio e conta as leituras.
pub fn varrer() -> Relatorio {
    varrer_em(&raiz(), ISENTOS)
}

/// O mesmo, a partir de uma raiz qualquer -- e o que deixa a prova real
/// plantar o defeito numa arvore fabricada, sem escrever `.rs` de mentira
/// dentro do repositorio.
pub fn varrer_em(raiz: &Path, isentos: &[(&str, usize, &str)]) -> Relatorio {
    let mut achados: Vec<Solta> = Vec::new();
    let mut fontes = Vec::new();
    for crate_ in ordenar(&raiz.join("crates")) {
        juntar_rs(&crate_.join("src"), &mut fontes);
    }
    for arq in fontes {
        let rel = arq
            .strip_prefix(raiz)
            .unwrap_or(&arq)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(texto) = std::fs::read_to_string(&arq) else {
            continue;
        };
        for (i, linha) in texto.lines().enumerate() {
            for _ in 0..leituras_na_linha(linha) {
                achados.push(Solta {
                    arquivo: rel.clone(),
                    linha: i + 1,
                    texto: linha.trim().to_string(),
                });
            }
        }
    }

    let mut r = Relatorio::default();
    for (arquivo, esperado, _) in isentos {
        let achado = achados.iter().filter(|s| s.arquivo == *arquivo).count();
        if achado != *esperado {
            r.isentos_mudaram
                .push((arquivo.to_string(), *esperado, achado));
        }
    }
    r.soltas = achados
        .into_iter()
        .filter(|s| !isentos.iter().any(|(a, _, _)| *a == s.arquivo))
        .collect();
    r
}

/// Os subdiretorios de `pasta`, em ordem -- relatorio que muda de ordem a
/// cada corrida nao se compara com o anterior.
fn ordenar(pasta: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

/// Junta todo `.rs` de `pasta` e das filhas dela.
fn juntar_rs(pasta: &Path, saida: &mut Vec<PathBuf>) {
    let mut entradas: Vec<PathBuf> = std::fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entradas.sort();
    for p in entradas {
        if p.is_dir() {
            juntar_rs(&p, saida);
        } else if p.extension().is_some_and(|e| e == "rs") {
            saida.push(p);
        }
    }
}

/// A catraca. **So desce.**
///
/// Nasceu no numero medido do dia do pedido 439 -- 1, o `email.rs` -- e
/// desceu a zero no mesmo conserto, que e o que o pedido mandava: nascer no
/// numero do dia, e nao no 5 do historico do 434, que seria subi-la no
/// proprio nascimento. Quem precisar de uma leitura de linha nova le pelo
/// `Canal`, ou entra em [`ISENTOS`] com o motivo escrito.
pub const TETO_LEITURA_FORA_DO_CANAL: usize = 0;

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    /// A catraca do pedido 439.
    ///
    /// O `<=` e proposital, pelo mesmo motivo do `conferidor_temporarios`: a
    /// lei e «catraca so desce», entao a comparacao aceita um numero MENOR
    /// que o teto. Ela so parece absurda hoje porque zero e o piso.
    #[test]
    #[allow(clippy::absurd_extreme_comparisons)]
    fn ninguem_le_linha_de_soquete_fora_do_canal() {
        let r = varrer();
        assert!(
            r.soltas.len() <= TETO_LEITURA_FORA_DO_CANAL,
            "{} leitura(s) de linha fora do `Canal` (teto {TETO_LEITURA_FORA_DO_CANAL}).\n\
             Leia pelo motor (`phxsql_core::fio::Canal::ler_ate`, com o teto dito em voz alta) \
             -- ou, se nao e soquete, entre em ISENTOS com o motivo escrito.\n{}",
            r.soltas.len(),
            r.soltas
                .iter()
                .map(|s| format!("  {}:{}  {}", s.arquivo, s.linha, s.texto))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// O outro lado do laco: o catalogo tambem envelhece.
    #[test]
    fn o_catalogo_de_isentos_da_leitura_bate_com_o_disco() {
        let r = varrer();
        assert!(
            r.isentos_mudaram.is_empty(),
            "o catalogo ISENTOS nao bate com o que esta no disco: {}",
            r.isentos_mudaram
                .iter()
                .map(|(a, e, c)| format!("{a}: esperava {e}, achou {c}"))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }

    /// **Prova real, numa arvore fabricada:** plantar a leitura crua sobe a
    /// conta de 0 para 1; o comentario que a descreve nao conta; e a leitura
    /// nova num arquivo isento aparece como isencao que mudou, e nao some.
    ///
    /// Numa arvore de mentira em `DirTemp`, e nao dentro de `crates/`: um
    /// teste que escrevesse `.rs` no repositorio estaria criando o lixo que o
    /// `conferidor_temporarios` existe para proibir.
    #[test]
    fn o_conferidor_acusa_a_leitura_plantada() {
        let d = DirTemp::novo("conferidor-canal");
        let src = d.join("crates").join("x").join("src");
        std::fs::create_dir_all(&src).unwrap();
        let isentos: &[(&str, usize, &str)] = &[("crates/x/src/motor.rs", 1, "o motor")];
        std::fs::write(
            src.join("motor.rs"),
            format!("fn f() {{ limitado{}&mut v); }}\n", PADROES[1]),
        )
        .unwrap();
        std::fs::write(
            src.join("cliente.rs"),
            format!(
                "// o defeito era leitor{}&mut l)\nfn g() {{}}\n",
                PADROES[0]
            ),
        )
        .unwrap();
        let r = varrer_em(&d, isentos);
        assert_eq!(r.soltas.len(), 0, "{:?}", r.soltas);
        assert!(r.isentos_mudaram.is_empty(), "{:?}", r.isentos_mudaram);

        // O defeito do pedido 439, plantado: a linha do rele lida crua.
        std::fs::write(
            src.join("cliente.rs"),
            format!("fn g() {{ leitor{}&mut linha); }}\n", PADROES[0]),
        )
        .unwrap();
        let r = varrer_em(&d, isentos);
        assert_eq!(r.soltas.len(), 1, "a leitura plantada nao foi contada");
        assert_eq!(r.soltas[0].arquivo, "crates/x/src/cliente.rs");

        // E a segunda leitura no arquivo ISENTO nao se esconde atras dele.
        std::fs::write(
            src.join("motor.rs"),
            format!(
                "fn f() {{ limitado{p}&mut v); }}\nfn h() {{ outro{p}&mut w); }}\n",
                p = PADROES[1]
            ),
        )
        .unwrap();
        let r = varrer_em(&d, isentos);
        assert_eq!(
            r.isentos_mudaram,
            vec![("crates/x/src/motor.rs".to_string(), 1, 2)]
        );
    }
}
