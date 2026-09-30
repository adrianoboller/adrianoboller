//! Pedido 511: o contador do `rowstamp` no teto.
//!
//! Arquivo proprio de proposito: o contador e do PROCESSO, e empurra-lo ao
//! teto dentro de um binario de testes compartilhado recusaria a insercao de
//! todo teste vizinho. Aqui ha um teste so, e o processo e dele.
//!
//! # Prova real
//!
//! Com o `fetch_add` de antes no `proximo_carimbo`, a terceira insercao sai
//! `Ok` com carimbo 0 em release -- o filho nascido «antes» do pai -- e entra
//! em panico em debug.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::PhxError;
use phxsql_store::table::{Table, Visao};

fn esquema() -> Schema {
    Schema::new(
        "pais",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn carimbo(t: &mut Table, rowid: u64) -> u64 {
    let i = t.esquema().coluna_rowstamp().unwrap();
    match &t.ler(rowid).unwrap().unwrap()[i] {
        Value::UInt(n) => *n,
        outro => panic!("rowstamp nao e UInt: {outro:?}"),
    }
}

#[test]
fn no_teto_o_carimbo_recusa_a_insercao_em_vez_de_dar_a_volta() {
    let dir = DirTemp::novo("carimbo-no-teto");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    let r1 = t.inserir(&[Value::Int(1)]).unwrap();
    // O que um evento replicado com carimbo no teto faz ao contador.
    phxsql_store::no::empurrar_carimbo(u64::MAX - 1);
    let r2 = t.inserir(&[Value::Int(2)]).unwrap();
    assert_eq!(carimbo(&mut t, r2), u64::MAX);
    assert!(carimbo(&mut t, r1) < carimbo(&mut t, r2));

    let e = t
        .inserir(&[Value::Int(3)])
        .expect_err("no teto a insercao tem de recusar, e nao dar a volta");
    assert!(matches!(e, PhxError::LimiteExcedido(_)), "{e}");
    assert!(e.to_string().contains("rowstamp"), "{e}");
    assert_eq!(phxsql_store::no::ultimo_carimbo(), u64::MAX);
    // Nada entrou: a recusa veio antes de gravar.
    assert_eq!(t.contar(Visao::Todas).unwrap(), 2);
}
