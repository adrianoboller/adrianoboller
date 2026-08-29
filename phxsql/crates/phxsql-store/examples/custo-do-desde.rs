//! Ler 500 eventos a partir da posicao P custa o mesmo para todo P?
//!
//! ```bash
//! cargo run --release --example custo-do-desde -- [eventos]
//! ```
//!
//! E a pergunta que a replicacao faz duzentas vezes seguidas: «me de 500
//! eventos a partir de P», com P andando de 500 em 500. Se o custo crescer com
//! P, alcancar N eventos custa N^2/2 e nao N -- e ai a replica nao fica para
//! tras por causa do que ela aplica, mas por causa do que o source varre.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);
    const LOTE: u64 = 500;

    let esquema = Schema::new(
        "t",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();

    let dir = std::env::temp_dir().join(format!("phx-desde-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    t.ligar_imagem_no_diario(true);
    for i in 1..=n as i64 {
        t.inserir(&[Value::Int(i), Value::Str(format!("Cliente {i:08}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();

    println!("=== ler {LOTE} eventos a partir de P, num diario de {n} ===\n");
    println!("  {:>10}  {:>10}  {:>12}", "P", "ms", "us/evento");

    let mut total = 0.0;
    for k in 0..10 {
        let p = (n as u64 / 10) * k;
        let inicio = Instant::now();
        let lidos = t.diario_com_imagem(p, LOTE).unwrap();
        let d = inicio.elapsed().as_secs_f64();
        println!(
            "  {p:>10}  {:>10.2}  {:>12.2}",
            d * 1e3,
            d * 1e6 / lidos.len().max(1) as f64
        );
    }

    // O que a replicacao paga de verdade: TODAS as leituras, de 0 ate o fim.
    let inicio = Instant::now();
    let mut p = 0u64;
    loop {
        let lidos = t.diario_com_imagem(p, LOTE).unwrap();
        if lidos.is_empty() {
            break;
        }
        p += lidos.len() as u64;
        total += 0.0;
    }
    let varrer_tudo = inicio.elapsed().as_secs_f64();
    let _ = total;

    println!(
        "\n  Alcancar os {n} eventos de {LOTE} em {LOTE}: {varrer_tudo:.2}s\
         \n  = {:.1} us por evento entregue, so no lado de quem serve.",
        varrer_tudo * 1e6 / n as f64
    );
    let _ = std::fs::remove_dir_all(&dir);
}
