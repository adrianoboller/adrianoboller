//! Sonda: um `fsync` no `.reg` acontece quando a janela fecha numa tabela que
//! foi escrita por OUTRA instancia de `Table` e depois reaberta?
//!
//! E o caminho exato de `descarregar_sujas_com`: quem escreveu foi um `Table`
//! que ja morreu, e quem sincroniza e um `Table` recem-aberto.
//!
//! ```bash
//! strace -f -y -e trace=fsync,openat target/release/examples/sonda-do-fecho
//! ```
//!
//! A cerca: o programa abre `/tmp/phx-cerca-<n>` entre as fases, entao o
//! `strace` mostra onde cada fase comeca. Contar `fsync` sem cerca mistura a
//! semeadura com o fecho, e foi assim que a primeira leitura deste numero quase
//! saiu errada.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn cerca(n: u32) {
    let _ = std::fs::File::open(format!("/tmp/phx-cerca-{n}"));
}

fn main() {
    let dir = std::env::temp_dir().join(format!("phx-sonda-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let esq = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let linha = |i: i64| vec![Value::Int(i), Value::Str(format!("nome {i}"))];

    // fase 1: nasce e sincroniza (estado limpo)
    {
        let mut t = Table::criar(&dir, esq).unwrap();
        for i in 1..=100 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    cerca(1);
    // fase 2: ESCREVE e morre sem sincronizar -- e o que `gravar_de_verdade`
    // faz quando a janela ainda nao fechou.
    {
        let mut t = Table::abrir(&dir, "clientes").unwrap();
        t.inserir(&linha(999_001)).unwrap();
    }

    cerca(2);
    // fase 3: o fecho da janela -- reabre e sincroniza, como o laco do servidor.
    {
        let mut t = Table::abrir(&dir, "clientes").unwrap();
        t.sincronizar().unwrap();
    }

    cerca(3);
    println!("sonda concluida em {}", dir.display());
    let _ = std::fs::remove_dir_all(&dir);
}
