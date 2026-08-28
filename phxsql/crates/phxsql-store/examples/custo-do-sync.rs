//! Quanto custa sincronizar a cada operacao, e o que a gravacao em lote daria.
//!
//! ```bash
//! cargo run --release --example custo-do-sync -- [linhas]
//! ```
//!
//! O servidor chama `sincronizar()` depois de cada `inserir`, `atualizar` e
//! `excluir`. Isso e `fsync` em ate cinco arquivos por linha gravada. Este
//! medidor separa as tres coisas que se confundem:
//!
//! 1. **inserir sem sincronizar** -- o custo de CPU do heap mais o indice;
//! 2. **inserir sincronizando a cada linha** -- o que o servidor faz hoje;
//! 3. **inserir e sincronizar de N em N** -- o *group commit*.
//!
//! A diferenca entre (1) e (2) e o preco da durabilidade por operacao. A
//! diferenca entre (2) e (3) e o que um lote devolveria.

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

fn esquema(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
            Column::new(
                "limite",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: u64) -> Vec<Value> {
    vec![
        Value::Int(i as i64),
        Value::Str(format!("Cliente {i:07}")),
        Value::Str(CIDADES[(i % 8) as usize].to_string()),
        Value::Decimal((i as i128 % 500_000) * 100),
    ]
}

/// Insere `n` linhas sincronizando de `lote` em `lote`. Lote 0 = so no fim.
fn medir(dir: &std::path::Path, nome: &str, n: u64, lote: u64) -> f64 {
    let mut t = Table::criar(dir, esquema(nome)).unwrap();
    let t0 = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
        if lote > 0 && i % lote == 0 {
            t.sincronizar().unwrap();
        }
    }
    t.sincronizar().unwrap();
    let s = t0.elapsed().as_secs_f64();
    // Nao mede a limpeza.
    for ext in ["reg", "ndx", "bin", "memo", "log"] {
        let _ = std::fs::remove_file(dir.join(format!("{nome}.{ext}")));
    }
    s
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);

    let dir = std::env::temp_dir().join(format!("phx-sync-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    println!("Custo do sincronizar, {n} linhas, mesma tabela e mesmos dados.\n");
    println!(
        "{:<34}{:>11}{:>13}{:>11}",
        "quando sincroniza", "segundos", "linhas/s", "vs. cada"
    );

    // Cada caso corre uma vez para aquecer o diretorio e uma vez para valer.
    let _ = medir(&dir, "aquece", 2_000, 0);

    let casos: [(&str, u64); 7] = [
        ("a cada linha (o servidor hoje)", 1),
        ("a cada 10", 10),
        ("a cada 100", 100),
        ("a cada 1.000", 1_000),
        ("a cada 10.000", 10_000),
        ("so no fim", 0),
        ("a cada linha, de novo", 1),
    ];
    let mut base = 0.0;
    for (i, (rot, lote)) in casos.iter().enumerate() {
        let s = medir(&dir, &format!("t{i}"), n, *lote);
        if i == 0 {
            base = s;
        }
        println!(
            "{:<34}{:>11.3}{:>13.0}{:>10.1}x",
            rot,
            s,
            n as f64 / s,
            base / s
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    println!("\nA ultima linha repete a primeira: se as duas nao baterem, a");
    println!("medida esta contaminada por cache do sistema de arquivos.");
}
