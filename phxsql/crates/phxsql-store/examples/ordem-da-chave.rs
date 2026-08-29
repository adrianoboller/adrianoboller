//! Quanto a ORDEM das chaves custa na insercao.
//!
//! ```bash
//! cargo run --release --example ordem-da-chave -- [linhas]
//! ```
//!
//! A hipotese do pedido 113 e que ordenar as chaves de um lote antes de
//! inserir no `.ndx` acelera a carga: chaves vizinhas caem na mesma folha da
//! B+tree, e a folha ja esta quente. A hipotese so vale se a desordem custar
//! alguma coisa -- e isso e o que este medidor descobre, ANTES de valer a pena
//! reescrever a insercao em lote.
//!
//! As mesmas linhas, os mesmos indices, a mesma quantidade. Muda so a ORDEM em
//! que as chaves chegam:
//!
//! - `crescente` -- a chave sobe junto com o rowid, que e o caso da carga de um
//!   arquivo ja ordenado. E o melhor caso possivel.
//! - `embaralhada` -- a mesma faixa de chaves, em ordem aleatoria. E o caso de
//!   uma carga vinda de um sistema que nao ordena, que e o comum.
//!
//! A diferenca entre as duas e o TETO do que ordenar o lote pode recuperar.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Xorshift64*: embaralha sem crate de fora.
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

fn colunas() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("codigo", ColumnType::Str(24)).obrigatoria(),
        Column::new("valor", ColumnType::Int8),
    ]
}

fn linha(id: i64) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(format!("COD{id:012}")),
        Value::Int(id * 7),
    ]
}

/// Insere `ids` na ordem dada e devolve os microssegundos por linha.
fn medir(rotulo: &str, ids: &[i64], indices: Vec<IndexDef>) -> f64 {
    let dir = std::env::temp_dir().join(format!("phx-ordem-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let esquema = Schema::new("itens", colunas(), indices).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();

    // Montadas antes do cronometro: formatar texto custa igual nas duas ordens
    // e nao e o que se quer medir.
    let linhas: Vec<Vec<Value>> = ids.iter().map(|&i| linha(i)).collect();

    let inicio = Instant::now();
    for l in &linhas {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    let s = inicio.elapsed().as_secs_f64();

    let paginas = t.paginas_indice();
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "  {rotulo:<26} {s:>7.2}s  {:>9.0} linhas/s  {:>7.1} us/linha  {paginas} paginas de indice",
        ids.len() as f64 / s,
        s * 1e6 / ids.len() as f64
    );
    s * 1e6 / ids.len() as f64
}

fn par(nome: &str, n: i64, indices: impl Fn() -> Vec<IndexDef>) {
    let crescente: Vec<i64> = (1..=n).collect();
    let mut embaralhada = crescente.clone();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for i in (1..embaralhada.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        embaralhada.swap(i, j);
    }

    println!("\n=== {nome} ===\n");
    let a = medir("crescente", &crescente, indices());
    let b = medir("embaralhada", &embaralhada, indices());
    println!(
        "\n  a desordem custa {:.2}x  ({:+.1} us por linha)",
        b / a,
        b - a
    );
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    println!("=== a ordem das chaves na insercao de {n} linhas ===");
    println!("\nA diferenca entre as duas linhas de cada bloco e o TETO do que");
    println!("ordenar o lote antes do `.ndx` pode recuperar.");

    par("um indice unico sobre a chave inteira", n, || {
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()]
    });

    par("um indice unico sobre a chave de texto", n, || {
        vec![IndexDef::new("porCodigo", vec![IndexColumn::asc(1)]).unico()]
    });

    par("a forma da bancada: dois indices", n, || {
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCodigo", vec![IndexColumn::asc(1)]),
        ]
    });
}
