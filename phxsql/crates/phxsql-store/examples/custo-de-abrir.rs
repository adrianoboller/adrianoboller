//! Quanto custa ABRIR a tabela, que e o que uma sessao de carga pagaria uma
//! vez em vez de por linha.
//!
//! ```bash
//! cargo run --release --example custo-de-abrir -- [linhas]
//! ```
//!
//! A carga em lote ja e 16x a carga linha a linha pela rede, e a explicacao
//! escrita ate aqui era "abrir a tabela, tomar a trava e sincronizar uma vez em
//! vez de por linha". Escrita, e nao medida: ninguem separou as tres.
//!
//! Este medidor separa a primeira. Ela e a que uma trava de sessao -- manter a
//! tabela aberta e reservada enquanto a carga dura -- eliminaria.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str("Blumenau".into()),
        Value::Memo(String::new()),
    ]
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);

    let dir = std::env::temp_dir().join(format!("phx-abrir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let mut t = Table::criar(&dir, esquema()).unwrap();
        for i in 1..=1_000 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    println!("=== o que uma sessao de carga deixaria de pagar por linha ===\n");

    // ------------------------------------------------- abrir e fechar
    let voltas = 20_000;
    let inicio = Instant::now();
    for _ in 0..voltas {
        let t = Table::abrir(&dir, "clientes").unwrap();
        std::hint::black_box(t.registros());
    }
    let abrir = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;
    println!("  abrir a tabela (7 arquivos) e fechar ....... {abrir:>8.2} us");

    // ------------------------------------------------- inserir, aberta uma vez
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    let base = t.registros() as i64;
    let inicio = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(base + i)).unwrap();
    }
    t.sincronizar().unwrap();
    let inserir = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("  inserir, com a tabela ja aberta ............ {inserir:>8.2} us");

    // ------------------------------------------------- inserir, abrindo sempre
    let base = t.registros() as i64;
    drop(t);
    let m = n.min(5_000);
    let inicio = Instant::now();
    for i in 1..=m {
        let mut t = Table::abrir(&dir, "clientes").unwrap();
        t.inserir(&linha(base + i)).unwrap();
        t.sincronizar().unwrap();
    }
    let por_vez = inicio.elapsed().as_secs_f64() * 1e6 / m as f64;
    println!("  inserir, abrindo e sincronizando por linha . {por_vez:>8.2} us");

    let _ = std::fs::remove_dir_all(&dir);

    println!("\n=== o veredito ===\n");
    println!(
        "  Abrir custa {:.1}x uma insercao. Numa carga linha a linha, cada linha\n  \
         paga {abrir:.0} us de abertura para {inserir:.0} us de trabalho de verdade.",
        abrir / inserir
    );
    println!(
        "\n  Uma sessao que mantem a tabela aberta e reservada tiraria isso do\n  \
         caminho: {por_vez:.0} -> {inserir:.0} us por linha, no motor, sem contar rede."
    );
}
