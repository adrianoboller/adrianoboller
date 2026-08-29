//! O cache de paginas do `.ndx` nascer vazio custa quanto por lote?
//!
//! ```bash
//! cargo run --release --example cache-frio -- [linhas_na_tabela] [lote]
//! ```
//!
//! A bancada carrega em lotes de 50.000 **abrindo e fechando a tabela em cada
//! lote** -- 200 processos para dez milhoes -- e a taxa dela cai de 54.180 para
//! 37.712 linhas/s. Num processo so a taxa NAO cai: 16,0 us por linha com 200
//! mil, 16,4 com seis milhoes.
//!
//! Consertar a abertura do `.reg`, que lia o arquivo inteiro, explicou parte da
//! diferenca e nao toda: sobraram ~6,6 us por linha. A suspeita seguinte, e o
//! que este medidor testa, e o **cache de paginas do `.ndx` nascer vazio a cada
//! processo**: quem roda um processo so mantem a raiz e os nos de cima quentes
//! do comeco ao fim; quem reabre a cada lote paga a releitura -- com o CRC-32
//! junto, que e o pedaco caro.
//!
//! Ele compara os dois no MESMO processo, para nao medir tambem o custo de
//! criar processo: um lote com o cache que veio do lote anterior, e um lote
//! logo depois de fechar e reabrir a tabela.

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
    ]
}

/// Insere `lote` linhas e devolve (segundos, acertos, lidas, gravadas).
fn um_lote(t: &mut Table, de: i64, lote: i64) -> (f64, u64, u64, u64) {
    let antes = t.estatisticas_paginas();
    let inicio = Instant::now();
    for i in de + 1..=de + lote {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let d = inicio.elapsed().as_secs_f64();
    let depois = t.estatisticas_paginas();
    (
        d,
        depois.0 - antes.0,
        depois.1 - antes.1,
        depois.2 - antes.2,
    )
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2_000_000);
    let lote: i64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);

    let dir = std::env::temp_dir().join(format!("phx-frio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut t = Table::criar(&dir, esquema()).unwrap();

    println!("=== cache quente contra cache frio, lotes de {lote} ===\n");
    println!(
        "  {:>10}  {:>8}  {:>10}  {:>9}  {:>9}  {:>9}",
        "ja tinha", "cache", "us/linha", "acertos", "lidas", "gravadas"
    );

    let mut feitas = 0i64;
    while feitas < n {
        // Lote com o cache herdado do lote anterior.
        let (q, qa, ql, qg) = um_lote(&mut t, feitas, lote);
        println!(
            "  {feitas:>10}  {:>8}  {:>10.2}  {:>9.2}  {:>9.2}  {:>9.2}",
            "quente",
            q * 1e6 / lote as f64,
            qa as f64 / lote as f64,
            ql as f64 / lote as f64,
            qg as f64 / lote as f64
        );
        feitas += lote;

        // Fecha e reabre: e o que a bancada faz a cada 50.000 linhas.
        drop(t);
        t = Table::abrir(&dir, "precos").unwrap();

        let (f, fa, fl, fg) = um_lote(&mut t, feitas, lote);
        println!(
            "  {feitas:>10}  {:>8}  {:>10.2}  {:>9.2}  {:>9.2}  {:>9.2}   {:+.1}%",
            "FRIO",
            f * 1e6 / lote as f64,
            fa as f64 / lote as f64,
            fl as f64 / lote as f64,
            fg as f64 / lote as f64,
            (f / q - 1.0) * 100.0
        );
        feitas += lote;
    }

    println!(
        "\n  «lidas» e o que teve de vir do ARQUIVO, e e o que paga CRC-32 a\
         \n  2,34 us a pagina. Se o lote frio ler mais que o quente, o custo de\
         \n  reabrir esta ai -- e ele nao aparece em quem roda um processo so."
    );
    let _ = std::fs::remove_dir_all(&dir);
}
