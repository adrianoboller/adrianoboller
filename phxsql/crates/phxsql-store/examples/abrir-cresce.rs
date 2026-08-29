//! Abrir a tabela custa mais conforme ela cresce?
//!
//! ```bash
//! cargo run --release --example abrir-cresce -- [linhas]
//! ```
//!
//! A pergunta saiu de um numero que nao fechava. Num processo so, inserir
//! custa 16,0 us por linha com 200 mil e **16,4 com seis milhoes** -- nao
//! degrada. Mas a bancada dos dez milhoes mostra a taxa caindo de 53.879 para
//! 36.517 linhas/s, e ela carrega em lotes de 50.000, **abrindo e fechando a
//! tabela em cada lote**: 200 processos para dez milhoes.
//!
//! Se abrir uma tabela grande custar mais do que abrir uma pequena, a queda e
//! da abertura e nao da insercao -- e importa muito, porque o servidor tambem
//! abre a tabela a cada pedido.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

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

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
    ]
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4_000_000);
    const PASSO: i64 = 500_000;
    const AMOSTRAS: u32 = 20;

    let dir = std::env::temp_dir().join(format!("phx-abrir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut t = Table::criar(&dir, esquema()).unwrap();

    println!("=== abrir a tabela, conforme ela cresce ===\n");
    println!(
        "  {:>12}  {:>10}  {:>14}  {:>16}",
        "linhas", "abrir", "por 50.000", "linhas/s efetivo"
    );

    let mut primeira_vez = true;
    let mut feitas = 0i64;
    while feitas < n {
        let ate = (feitas + PASSO).min(n);
        for i in feitas + 1..=ate {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
        feitas = ate;
        drop(t);

        // Media de varias aberturas: uma so pega o cache do sistema num estado
        // qualquer.
        let inicio = Instant::now();
        for _ in 0..AMOSTRAS {
            let aberta = Table::abrir(&dir, "precos").unwrap();
            drop(aberta);
        }
        let abrir_ms = inicio.elapsed().as_secs_f64() * 1e3 / AMOSTRAS as f64;

        // Qual dos sete arquivos cresce? Cada um medido sozinho.
        let mut por_arquivo = Vec::new();
        macro_rules! medir {
            ($rotulo:expr, $expr:expr) => {{
                let i = Instant::now();
                for _ in 0..AMOSTRAS {
                    let x = $expr;
                    drop(x);
                }
                por_arquivo.push(($rotulo, i.elapsed().as_secs_f64() * 1e3 / AMOSTRAS as f64));
            }};
        }
        let pag = phxsql_store::reg::RegFile::abrir(&dir, "precos")
            .unwrap()
            .esquema()
            .paginacao();
        let ext = pag.para_externos();
        medir!(
            ".reg",
            phxsql_store::reg::RegFile::abrir(&dir, "precos").unwrap()
        );
        medir!(
            ".ndx",
            phxsql_store::ndx::NdxFile::abrir(dir.join("precos.ndx")).unwrap()
        );
        medir!(
            ".log",
            phxsql_store::log::LogFile::abrir(&dir, "precos", ext).unwrap()
        );
        if primeira_vez {
            println!("  (uma vez) tempo de abrir cada arquivo, isolado:");
        }
        let detalhe: Vec<String> = por_arquivo
            .iter()
            .map(|(r, ms)| format!("{r} {ms:.2}ms"))
            .collect();
        println!("      {}", detalhe.join("   "));
        primeira_vez = false;

        // O que a bancada paga: uma abertura a cada 50.000 linhas, somada aos
        // 16,4 us por linha que a insercao custa em regime.
        let por_linha_us = abrir_ms * 1e3 / 50_000.0 + 16.4;
        println!(
            "  {feitas:>12}  {abrir_ms:>8.2} ms  {:>12.2} us  {:>16.0}",
            abrir_ms * 1e3 / 50_000.0,
            1e6 / por_linha_us
        );

        t = Table::abrir(&dir, "precos").unwrap();
    }

    println!(
        "\n  «por 50.000» reparte o custo de UMA abertura pelas 50.000 linhas do\
         \n  lote da bancada; «linhas/s efetivo» soma isso aos 16,4 us por linha\
         \n  que a insercao custa em regime, num processo so."
    );
    let _ = std::fs::remove_dir_all(&dir);
}
