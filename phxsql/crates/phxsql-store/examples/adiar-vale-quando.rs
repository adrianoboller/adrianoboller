//! Adiar o indice nao unico numa carga compensa a partir de que tamanho?
//!
//! ```bash
//! cargo run --release --example adiar-vale-quando -- [linhas_na_tabela]
//! ```
//!
//! `--example indice-adiado` mede o caso da tabela VAZIA, e la adiar o indice
//! nao unico vale 1,59x. Mas o `reindexar` reconstroi o indice sobre a tabela
//! INTEIRA, e nao sobre as linhas que acabaram de entrar: numa tabela que ja
//! tem cinco milhoes de linhas, carregar mil e depois reconstruir sobre
//! 5.001.000 e obviamente pior do que manter o indice nas mil.
//!
//! Existe entao um ponto de virada, e ele decide se vale a pena mexer no
//! formato para marcar indice suspenso. Este medidor o procura, em vez de o
//! deduzir: para uma tabela com N linhas, carrega M e cronometra os dois
//! caminhos.

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

fn colunas() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("produto", ColumnType::Str(40)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(20)),
    ]
}

fn unico() -> IndexDef {
    IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()
}

fn nao_unico() -> IndexDef {
    IndexDef::new("porCidade", vec![IndexColumn::asc(2)])
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
    ]
}

fn dir_limpo(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-quando-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Carrega `m` linhas numa tabela que ja tem `n`, com os dois indices vivos.
fn manter(n: i64, m: i64) -> f64 {
    let dir = dir_limpo("manter");
    let esquema = Schema::new("t", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();

    let inicio = Instant::now();
    for i in n + 1..=n + m {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let d = inicio.elapsed().as_secs_f64();
    let _ = std::fs::remove_dir_all(&dir);
    d
}

/// O mesmo, mas o nao unico fica de fora da carga e e reconstruido no fim.
///
/// A reconstrucao usada aqui e `reindexar`, que refaz TODOS os indices sobre a
/// tabela inteira -- e e justamente esse "sobre a tabela inteira" que faz o
/// ponto de virada existir.
fn adiar(n: i64, m: i64) -> (f64, f64) {
    let dir = dir_limpo("adiar");
    let esquema = Schema::new("t", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();

    // Simula a carga com o nao unico parado. Como nao ha (ainda) indice
    // suspenso no formato, a tabela e recriada so com o unico e recarregada --
    // o que conta e o TEMPO da carga com um indice em vez de dois.
    let dir2 = dir_limpo("adiar-carga");
    let esquema2 = Schema::new("t", colunas(), vec![unico()]).unwrap();
    let mut t2 = Table::criar(&dir2, esquema2).unwrap();
    for i in 1..=n {
        t2.inserir(&linha(i)).unwrap();
    }
    t2.sincronizar().unwrap();

    let inicio = Instant::now();
    for i in n + 1..=n + m {
        t2.inserir(&linha(i)).unwrap();
    }
    t2.sincronizar().unwrap();
    let carga = inicio.elapsed().as_secs_f64();

    // E a reconstrucao, sobre as n+m linhas, na tabela com os dois indices.
    for i in n + 1..=n + m {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let inicio = Instant::now();
    t.reindexar().unwrap();
    t.sincronizar().unwrap();
    let refazer = inicio.elapsed().as_secs_f64();

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
    (carga, refazer)
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    println!("=== tabela com {n} linhas; carregar M e adiar o indice nao unico ===\n");
    println!(
        "  {:>9}  {:>8}  {:>8} + {:>8} = {:>8}  {:>7}",
        "M", "manter", "carga", "refazer", "adiado", "ganho"
    );

    for div in [1i64, 2, 5, 10, 50] {
        let m = (n / div).max(1);
        let manter_s = manter(n, m);
        let (carga, refazer) = adiar(n, m);
        let adiado = carga + refazer;
        println!(
            "  {m:>9}  {manter_s:>7.3}s  {carga:>7.3}s + {refazer:>7.3}s = {adiado:>7.3}s  {:>6.2}x",
            manter_s / adiado
        );
    }

    println!(
        "\n  «refazer» e o `reindexar`, e ele varre a tabela INTEIRA -- por isso\
         \n  ele nao encolhe quando M encolhe. Ganho abaixo de 1,00x quer dizer\
         \n  que adiar CUSTOU tempo, e nao economizou."
    );
}
