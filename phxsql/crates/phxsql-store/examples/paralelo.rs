//! Quanto a varredura em memoria ganha dividindo entre nucleos.
//!
//! ```bash
//! cargo run --release --example paralelo -- [linhas]
//! ```
//!
//! Mede a consulta que NAO tem atalho de mapa -- a que precisa olhar linha por
//! linha. E o unico trecho do motor que divide bem: tudo esta em RAM, nada e
//! gravado, e cada linha e uma pergunta que nao depende das outras.
//!
//! Para ver o antes-e-depois, suba `MINIMO_PARA_DIVIDIR` em
//! `phxsql-core/src/paralelo.rs` acima do numero de linhas e rode de novo: isso
//! forca o caminho sequencial sem mudar mais nada.
//!
//! O que a medicao mostrou em 4 nucleos, com 1.000.000 de linhas: 36 ms
//! sequencial contra 20 ms paralelo -- **1,8x, nao 4x**. A varredura e presa a
//! banda de memoria, nao a conta: o filtro por linha e barato, e mais nucleo
//! nao compra mais banda.

use std::time::Instant;

use phxsql_core::paralelo::nucleos;
use phxsql_core::schema::{Column, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::memoria::{Consulta, Filtro, Operador, TabelaMemoria};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    let dir = std::env::temp_dir().join(format!("phx-paralelo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;

    let esquema = Schema::new(
        "precos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)),
            Column::new("cidade", ColumnType::Str(20)),
            Column::new("valor", ColumnType::Int8),
        ],
        vec![],
    )?;
    let mut t = Table::criar(&dir, esquema)?;

    println!("gravando {n} linhas...");
    for i in 1..=n {
        t.inserir(&[
            Value::Int(i),
            Value::Str(format!("Produto {i:08}")),
            Value::Str(CIDADES[(i as usize) % 8].into()),
            Value::Int(i % 100_000),
        ])?;
    }
    t.sincronizar()?;

    let inicio = Instant::now();
    let m = TabelaMemoria::carregar(&mut t, &[], 0)?;
    println!(
        "carregada em {:.2}s · {} nucleos disponiveis\n",
        inicio.elapsed().as_secs_f64(),
        nucleos()
    );

    // Operador Maior nao tem mapa de igualdade: forca a varredura inteira.
    let consulta = Consulta {
        onde: vec![Filtro {
            coluna: 3,
            op: Operador::Maior,
            valor: Value::Int(99_000),
        }],
        ordenar: vec![],
        colunas: vec![],
        pular: 0,
        max: 0,
    };

    // Cinco voltas: a primeira paga o aquecimento das caches.
    let mut melhor = f64::MAX;
    for volta in 1..=5 {
        let inicio = Instant::now();
        let r = m.selecionar(&consulta)?;
        let s = inicio.elapsed().as_secs_f64();
        melhor = melhor.min(s);
        println!(
            "  volta {volta}: {:>6.1} ms   {} achadas de {} examinadas",
            s * 1000.0,
            r.achadas,
            r.examinadas
        );
    }
    println!(
        "\n  melhor: {:.1} ms · {:.1} milhoes de linhas por segundo",
        melhor * 1000.0,
        n as f64 / melhor / 1e6
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
