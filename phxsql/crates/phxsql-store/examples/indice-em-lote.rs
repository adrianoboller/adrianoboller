//! Construir a B+tree de uma vez custa quanto, contra chave a chave?
//!
//! ```bash
//! cargo run --release --example indice-em-lote -- [linhas]
//! ```
//!
//! `--example indice-adiado` mediu o PISO teorico desta ideia (varrer,
//! codificar, ordenar) e disse que era 0,24 s contra os 2,54 s que o
//! `reindexar` chave a chave cobrava. Este mede a coisa pronta.
//!
//! E mede tambem o que a construcao em lote obriga a escolher e que a insercao
//! uma a uma nao escolhe: **quanto de cada folha encher**. Inserir em ordem
//! aleatoria assenta perto de 69% de ocupacao sozinho -- e um resultado
//! classico de B-tree --, entao encher a 70% nao compra compactacao nenhuma.
//! Encher a 100% da a arvore menor e a varredura mais rapida, e cobra uma
//! divisao na PRIMEIRA insercao seguinte em cada folha. A tabela abaixo tem as
//! duas pontas.

use std::time::Instant;

use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::NdxFile;

/// Xorshift64*, para embaralhar sem crate de fora.
struct Rng(u64);

impl Rng {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn esquema() -> Schema {
    Schema::new(
        "t",
        vec![Column::new("k", ColumnType::Int8)],
        vec![IndexDef::new("porChave", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn chave(v: i64) -> Vec<u8> {
    let ty = ColumnType::Int8;
    let mut buf = vec![0u8; largura_componente(&ty).unwrap()];
    escrever_componente(&Value::Int(v), &ty, false, false, &mut buf).unwrap();
    buf
}

fn dir_limpo(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-lote-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Medida {
    construir: f64,
    paginas: u64,
    varrer: f64,
    crescer: f64,
    paginas_depois: u64,
}

/// Insere as `n` chaves embaralhadas uma a uma, e depois faz a tabela crescer.
fn uma_a_uma(embaralhadas: &[i64], depois: &[i64]) -> Medida {
    let dir = dir_limpo("uma");
    let mut n = NdxFile::criar(dir.join("t.ndx"), &esquema()).unwrap();

    let inicio = Instant::now();
    for (i, v) in embaralhadas.iter().enumerate() {
        n.inserir(0, &chave(*v), i as u64 + 1).unwrap();
    }
    n.sincronizar().unwrap();
    let construir = inicio.elapsed().as_secs_f64();
    let paginas = n.paginas();

    let inicio = Instant::now();
    let lidas = n.varrer(0).unwrap();
    let varrer = inicio.elapsed().as_secs_f64();
    assert_eq!(lidas.len(), embaralhadas.len());

    let base = embaralhadas.len() as u64;
    let inicio = Instant::now();
    for (i, v) in depois.iter().enumerate() {
        n.inserir(0, &chave(*v), base + i as u64 + 1).unwrap();
    }
    n.sincronizar().unwrap();
    let crescer = inicio.elapsed().as_secs_f64();
    let paginas_depois = n.paginas();

    let _ = std::fs::remove_dir_all(&dir);
    Medida {
        construir,
        paginas,
        varrer,
        crescer,
        paginas_depois,
    }
}

/// Monta a arvore de uma vez, com o enchimento dado, e depois faz crescer.
fn em_lote(embaralhadas: &[i64], depois: &[i64], enchimento: usize) -> Medida {
    let dir = dir_limpo(&format!("lote{enchimento}"));
    let mut n = NdxFile::criar(dir.join("t.ndx"), &esquema()).unwrap();

    // O buffer plano faz parte do custo: e o que `reindexar` monta varrendo.
    let inicio = Instant::now();
    let mut buf = Vec::with_capacity(embaralhadas.len() * 16);
    for (i, v) in embaralhadas.iter().enumerate() {
        buf.extend_from_slice(&NdxFile::chave_completa(&chave(*v), i as u64 + 1));
    }
    n.construir_em_lote_com(0, buf, enchimento).unwrap();
    n.sincronizar().unwrap();
    let construir = inicio.elapsed().as_secs_f64();
    let paginas = n.paginas();

    let inicio = Instant::now();
    let lidas = n.varrer(0).unwrap();
    let varrer = inicio.elapsed().as_secs_f64();
    assert_eq!(lidas.len(), embaralhadas.len());

    let base = embaralhadas.len() as u64;
    let inicio = Instant::now();
    for (i, v) in depois.iter().enumerate() {
        n.inserir(0, &chave(*v), base + i as u64 + 1).unwrap();
    }
    n.sincronizar().unwrap();
    let crescer = inicio.elapsed().as_secs_f64();
    let paginas_depois = n.paginas();

    let _ = std::fs::remove_dir_all(&dir);
    Medida {
        construir,
        paginas,
        varrer,
        crescer,
        paginas_depois,
    }
}

fn linha(rotulo: &str, m: &Medida, base: f64, n: usize, cresceu: usize) {
    println!(
        "  {rotulo:<22} {:>7.3}s {:>6.2}x  {:>8} pag  {:>7.1} ch/pag  {:>7.3}s  {:>7.3}s  {:>+6}",
        m.construir,
        base / m.construir,
        m.paginas,
        n as f64 / m.paginas as f64,
        m.varrer,
        m.crescer,
        m.paginas_depois as i64 - m.paginas as i64,
    );
    let _ = cresceu;
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    // As chaves ficam embaralhadas: o caso comum, e o unico em que a ordem da
    // insercao importa.
    //
    // As que entram DEPOIS sao 10% do total e caem NO MEIO da faixa, e nao
    // acima dela: chave maior que todas vai sempre para a ultima folha, e ai a
    // divisao que o enchimento deveria provocar nunca acontece -- foi assim que
    // a primeira versao deste medidor deu 100% de graca. As de tras sao pares,
    // as de crescer sao impares.
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut valores: Vec<i64> = (1..=n as i64).map(|i| i * 2).collect();
    for i in (1..valores.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        valores.swap(i, j);
    }
    let cresceu = n / 10;
    let mut depois: Vec<i64> = (0..cresceu)
        .map(|i| (i as i64 * 10 + 1) % (n as i64 * 2))
        .collect();
    depois.sort_unstable();
    depois.dedup();
    for i in (1..depois.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        depois.swap(i, j);
    }
    let cresceu = depois.len();

    println!("=== construir o indice de {n} chaves ===\n");
    println!(
        "  {:<22} {:>7} {:>6}   {:>8}      {:>7}      {:>7}  {:>7}  {:>6}",
        "", "montar", "ganho", "paginas", "ocupacao", "varrer", "crescer", "+pag"
    );

    let uma = uma_a_uma(&valores, &depois);
    let base = uma.construir;
    linha("uma a uma (hoje)", &uma, base, n, cresceu);

    for e in [70usize, 80, 90, 95, 100] {
        let m = em_lote(&valores, &depois, e);
        linha(&format!("em lote, {e}% cheio"), &m, base, n, cresceu);
    }

    println!(
        "\n  «crescer» insere mais {cresceu} chaves DEPOIS de a arvore estar\
         \n  pronta, e «+pag» diz quantas paginas isso custou. E ai que o\
         \n  enchimento se paga ou se cobra: folha cheia divide na primeira\
         \n  insercao que cair nela.\
         \n\n  «ocupacao» e quantas chaves cabem por pagina do arquivo INTEIRO,\
         \n  nos internos junto -- por isso ela nao bate com o enchimento pedido."
    );
}
