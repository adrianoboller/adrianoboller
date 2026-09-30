//! O `adiar_indice` do `BULKINSERT` (pedido 324) medido pelo MOTOR, e nao
//! pela equivalencia do `bulkinsert-adiar-ndx`.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! target/release/examples/indice-adiado-de-verdade <n>
//! ```
//!
//! Aquele medidor carregava numa tabela SEM indice declarado e cronometrava o
//! `reindexar` numa outra, porque o motor ainda nao sabia suspender. Este usa
//! o caminho que o servidor usa -- `Table::adiar_indice`, `inserir`,
//! `reindexar`, `sincronizar` -- e por isso mede tambem o que a equivalencia
//! nao media: as chaves tiradas por linha mesmo com o indice adiado (e ali
//! que a linha que nao cabe no indice e recusada).
//!
//! E mede a forma que a R1 permite: DOIS indices NAO unicos (`porId`,
//! `porCidade`). O 2,10x de 17/09/2026 tinha um unico entre os dois, e o
//! indice unico nao se adia.
//!
//! Uma linha `RESULTADO {json}` por chamada; a faixa min-max sai de VARIAS
//! chamadas de processo. Antes de apagar, confere que os dois regimes
//! terminam no MESMO estado -- busca id a id e contagem por cidade -- e
//! imprime `VERIFICACAO ok`, ou explode dizendo o que divergiu.

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
        "carga",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

/// As N linhas em ordem EMBARALHADA (xorshift64*, a mesma do
/// `bulkinsert-adiar-ndx`): carga de verdade nao chega ordenada.
fn linhas(n: i64) -> Vec<Vec<Value>> {
    let mut ids: Vec<i64> = (1..=n).collect();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in (1..ids.len()).rev() {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let j = (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % (i as u64 + 1)) as usize;
        ids.swap(i, j);
    }
    ids.into_iter()
        .map(|i| {
            vec![
                Value::Int(i),
                Value::Str(format!("Produto {i:08}")),
                Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
            ]
        })
        .collect()
}

fn dir(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-324-medir-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);
    let ls = linhas(n);

    let d_linha = dir("em-linha");
    let mut em_linha = Table::criar(&d_linha, esquema()).unwrap();
    let t0 = Instant::now();
    for l in &ls {
        em_linha.inserir(l).unwrap();
    }
    em_linha.sincronizar().unwrap();
    let s_linha = t0.elapsed().as_secs_f64();

    let d_adiado = dir("adiado");
    let mut adiado = Table::criar(&d_adiado, esquema()).unwrap();
    let t0 = Instant::now();
    adiado.adiar_indice().unwrap();
    for l in &ls {
        adiado.inserir(l).unwrap();
    }
    let s_carga = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    adiado.reindexar().unwrap();
    adiado.sincronizar().unwrap();
    let s_refazer = t1.elapsed().as_secs_f64();

    // O mesmo estado, antes de publicar numero nenhum.
    assert_eq!(em_linha.registros(), adiado.registros());
    for i in 1..=n {
        let a = em_linha.buscar("porId", &[Value::Int(i)]).unwrap();
        let b = adiado.buscar("porId", &[Value::Int(i)]).unwrap();
        assert_eq!(a, b, "porId diverge em {i}");
        assert_eq!(a.len(), 1, "id {i}");
    }
    for c in CIDADES {
        let mut a = em_linha
            .buscar("porCidade", &[Value::Str(c.into())])
            .unwrap();
        let mut b = adiado.buscar("porCidade", &[Value::Str(c.into())]).unwrap();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "porCidade diverge em {c}");
    }
    adiado.verificar().unwrap();
    eprintln!("VERIFICACAO ok");

    println!(
        "RESULTADO {{\"n\":{n},\"em_linha_s\":{s_linha:.4},\"adiado_carga_s\":{s_carga:.4},\
         \"adiado_refazer_s\":{s_refazer:.4},\"adiado_total_s\":{:.4}}}",
        s_carga + s_refazer
    );
    drop(em_linha);
    drop(adiado);
    let _ = std::fs::remove_dir_all(&d_linha);
    let _ = std::fs::remove_dir_all(&d_adiado);
}
