//! A tabela ABERTA insere na mesma velocidade que a tabela recem-CRIADA?
//!
//! ```bash
//! cargo run --release --example abrir-contra-criar -- [linhas]
//! ```
//!
//! A pergunta nasceu de dois medidores honestos discordando por 2x: o `carga`
//! (que abre uma tabela criada por outro processo) mede 16,9 us/linha, e o
//! `custo-do-fsync` (que cria a tabela no proprio processo) mede 8,0 -- mesmo
//! esquema, mesma maquina quieta. A unica diferenca de caminho e
//! `Table::abrir` contra `Table::criar`. Este medidor isola exatamente isso.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

fn esquema() -> Schema {
    Schema::new(
        "precos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
            Column::new(
                "valor",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("cadastro", ColumnType::Date),
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
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
        Value::Decimal(((i % 900_000) + 100) as i128),
        Value::Date(20_000 + (i % 400) as i32),
    ]
}

fn medir(rotulo: &str, mut t: Table, n: i64) {
    let inicio = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let s = inicio.elapsed().as_secs_f64();
    println!(
        "  {rotulo:<22} {s:>7.2}s  {:>9.0} linhas/s  {:>6.2} us/linha",
        n as f64 / s,
        s * 1e6 / n as f64
    );
}

fn dir_limpo(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-avc-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    println!("=== inserir {n} linhas: a tabela criada aqui contra a aberta ===\n");

    // Criada no proprio processo.
    let d1 = dir_limpo("criada");
    medir("recem-criada", Table::criar(&d1, esquema()).unwrap(), n);

    // Criada, fechada e REABERTA -- o caminho do `carga` e do servidor.
    let d2 = dir_limpo("aberta");
    drop(Table::criar(&d2, esquema()).unwrap());
    medir("criada e reaberta", Table::abrir(&d2, "precos").unwrap(), n);

    let _ = std::fs::remove_dir_all(&d1);
    let _ = std::fs::remove_dir_all(&d2);
}
