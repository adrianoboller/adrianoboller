//! Quanto custa CONTAR as orfas na replica -- pedido 300 §2.7.
//!
//! ```bash
//! cargo run --release --example custo-das-orfas -p phxsql-store -- [eventos]
//! ```
//!
//! A replica aplica os eventos de uma filha com chave conferida, pelo
//! `aplicar_evento`, de tres jeitos: sem contar (o de antes), contando com a
//! mae presente (cada linha acha a sua) e contando com a mae ausente (cada
//! linha e orfa). A conta da triagem dizia «da ordem de um incremento»; a
//! conferencia, na verdade, e uma busca no indice da mae por linha -- e e
//! isso que se mede aqui, em vez de citar.

use std::path::Path;
use std::time::Instant;

use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn mae() -> Schema {
    Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn filha() -> Schema {
    Schema::new(
        "itens",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("cliente", ColumnType::Int8),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )])
    .unwrap()
}

/// Uma replica nova em `dir`: a mae com `n` linhas (ou vazia) e a filha vazia.
fn replica(dir: &Path, n: i64, com_mae: bool) -> Table {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let mut m = Table::criar(dir, mae()).unwrap();
    if com_mae {
        for i in 1..=n {
            m.inserir(&[Value::Int(i)]).unwrap();
        }
    }
    m.sincronizar().unwrap();
    drop(m);
    Table::criar(dir, filha()).unwrap()
}

/// Aplica os eventos em lotes de 500 -- o servidor reabre a tabela por lote,
/// e e por lote que a contagem abre a mae.
fn aplicar(
    dir: &Path,
    eventos: &[(phxsql_store::log::Evento, Vec<u8>)],
    contar: bool,
) -> (f64, (u64, u64)) {
    let mut total = (0, 0);
    let inicio = Instant::now();
    for lote in eventos.chunks(500) {
        let mut t = Table::abrir(dir, "itens").unwrap();
        if contar {
            t.contar_orfas();
        }
        for (ev, imagem) in lote {
            t.aplicar_evento(ev.operacao, ev.rowid, imagem).unwrap();
        }
        let (o, s) = t.orfas_contadas();
        total = (total.0 + o, total.1 + s);
    }
    (inicio.elapsed().as_secs_f64(), total)
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);
    let raiz = std::env::temp_dir().join(format!("phx-orfas-{}", std::process::id()));
    let fonte = raiz.join("fonte");
    let mut f = replica(&fonte, n, true);
    f.ligar_imagem_no_diario(true);
    for i in 1..=n {
        f.inserir(&[Value::Int(i), Value::Int(i)]).unwrap();
    }
    f.sincronizar().unwrap();
    let eventos = f.diario_com_imagem_ate(0, n as u64, usize::MAX).unwrap();
    drop(f);

    println!("=== contar orfas na replica: {n} eventos de uma filha com chave conferida ===");
    for (rotulo, contar, com_mae) in [
        ("sem contar (antes)", false, true),
        ("contando, mae presente", true, true),
        ("contando, mae ausente", true, false),
    ] {
        let dir = raiz.join("replica");
        drop(replica(&dir, n, com_mae));
        let (d, (o, s)) = aplicar(&dir, &eventos, contar);
        println!(
            "{rotulo:<24} {:>8.1} ms  {:>6.2} us/evento  orfas {o}  sem_conferir {s}",
            d * 1e3,
            d * 1e6 / n as f64
        );
    }
    let _ = std::fs::remove_dir_all(&raiz);
}
