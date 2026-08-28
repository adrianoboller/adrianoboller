//! Onde a insercao gasta o tempo. Um fator por vez.
//!
//! ```bash
//! cargo run --release --example onde-doi -- [linhas]
//! ```
//!
//! A bancada diz QUE a insercao e lenta (248 us por linha, 99% de CPU, disco
//! parado). Nao diz ONDE. Este medidor separa as parcelas montando a mesma
//! tabela com esquemas diferentes e inserindo as mesmas linhas em cada um:
//!
//! - `so .reg` -- sem indice nenhum: o custo do heap mais o diario
//! - `+1 indice` -- um indice comum
//! - `+1 unico` -- o mesmo indice, agora unico. A diferenca e a busca que toda
//!   insercao faz antes de gravar, para conferir a chave
//! - `+2 indices` -- a forma da bancada, com o segundo indice de baixa
//!   cardinalidade
//!
//! A conta de cada parcela sai da subtracao, e o resto do relatorio mede
//! diretamente as duas suspeitas que aparecem no caminho de cada pagina do
//! `.ndx`: o CRC-32 da pagina inteira e a chamada de sistema.

use std::time::Instant;

use phxsql_core::crc::crc32;
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

fn colunas() -> Vec<Column> {
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
    ]
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

fn medir(rotulo: &str, indices: Vec<IndexDef>, n: i64) -> f64 {
    let dir = std::env::temp_dir().join(format!("phx-onde-doi-{}-{}", std::process::id(), rotulo));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let esquema = Schema::new("precos", colunas(), indices).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();

    // As linhas sao montadas ANTES do cronometro: o custo de formatar texto e
    // o mesmo em todas as variantes e nao e o que se quer medir.
    let linhas: Vec<Vec<Value>> = (1..=n).map(linha).collect();

    let inicio = Instant::now();
    for l in &linhas {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    let s = inicio.elapsed().as_secs_f64();

    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "  {rotulo:<14} {:>8.2}s  {:>9.0} linhas/s  {:>7.1} us por linha",
        s,
        n as f64 / s,
        s * 1e6 / n as f64
    );
    s * 1e6 / n as f64
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);

    println!("=== insercao de {n} linhas, um fator por vez ===\n");

    let so_reg = medir("so .reg", vec![], n);
    let um = medir(
        "+1 indice",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])],
        n,
    );
    let um_unico = medir(
        "+1 unico",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        n,
    );
    let dois = medir(
        "+2 indices",
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
        n,
    );

    println!("\n=== o que cada parcela custa, por linha ===\n");
    println!(
        "  .reg + .log ................ {so_reg:>7.1} us   {:>5.1}%",
        so_reg / dois * 100.0
    );
    println!(
        "  primeiro indice ............ {:>7.1} us   {:>5.1}%",
        um - so_reg,
        (um - so_reg) / dois * 100.0
    );
    println!(
        "  conferir a chave unica ..... {:>7.1} us   {:>5.1}%",
        um_unico - um,
        (um_unico - um) / dois * 100.0
    );
    println!(
        "  segundo indice ............. {:>7.1} us   {:>5.1}%",
        dois - um_unico,
        (dois - um_unico) / dois * 100.0
    );
    println!("  {:-<31} {dois:>7.1} us   100.0%", " TOTAL ");

    // --------------------------------------------------------------- CRC
    // Toda leitura e toda gravacao de pagina do `.ndx` passa a pagina inteira
    // pelo CRC-32. Quanto isso custa, isolado?
    let pagina = vec![0x5Au8; 4096];
    let voltas = 200_000;
    let inicio = Instant::now();
    let mut acc = 0u32;
    for _ in 0..voltas {
        acc = acc.wrapping_add(crc32(&pagina));
    }
    let por_pagina = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;
    println!("\n=== as duas suspeitas do caminho de cada pagina ===\n");
    println!("  CRC-32 de uma pagina de 4 KiB .... {por_pagina:.2} us   (acumulador {acc:x})");

    // --------------------------------------------------- chamada de sistema
    // Um `lseek` num arquivo ja aberto e a chamada mais barata que o caminho
    // faz. Serve de piso para o custo de ir ao nucleo.
    use std::io::{Seek, SeekFrom};
    let alvo = std::env::temp_dir().join(format!("phx-seek-{}", std::process::id()));
    let mut f = std::fs::File::create(&alvo).unwrap();
    let inicio = Instant::now();
    for i in 0..voltas {
        f.seek(SeekFrom::Start((i % 4096) as u64)).unwrap();
    }
    let por_seek = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;
    let _ = std::fs::remove_file(&alvo);
    println!("  um lseek .......................... {por_seek:.2} us");

    println!(
        "\n  O `strace` conta 41 chamadas e ~20 toques de pagina por linha inserida.\n  \
         Isso da ~{:.0} us so de nucleo e ~{:.0} us so de CRC -- de {dois:.0} us medidos.",
        41.0 * por_seek,
        20.0 * por_pagina
    );
}
