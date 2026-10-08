//! Pedido 601: a replica FIEL confere a linhagem antes do primeiro evento.
use super::*;
use phxsql_core::schema::{Column, IndexColumn, IndexDef};
use phxsql_core::types::ColumnType;

fn clientes() -> Schema {
    Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn no(esquema: Schema) -> crate::replica::NoSource {
    crate::replica::NoSource {
        nome: "clientes".into(),
        eventos: 0,
        esquema: Some(esquema),
        proxima_sequencia: 0,
    }
}

/// A tabela daqui e de OUTRA historia que a do source: recusa, e nomeia a
/// linhagem e o que fazer. A da MESMA historia, e a do source de antes da
/// v11 (sem linhagem), seguem como sempre.
///
/// **Defeito reposto**: o `recusa_da_linhagem` sem conferir -- a de outra
/// historia segue, e a replica aplicaria os eventos dela no rowid de
/// outra linha.
#[test]
fn a_replica_recusa_a_tabela_de_outra_historia() {
    let aqui = clientes();
    let motivo = recusa_da_linhagem(&aqui, "loja", &no(clientes()))
        .expect("a replica seguiu a tabela de outra historia");
    for pedaco in [
        "linhagem",
        "historia",
        "apagada e recriada",
        "loja.clientes",
    ] {
        assert!(
            motivo.contains(pedaco),
            "a recusa nao diz {pedaco:?}: {motivo}"
        );
    }
    assert_eq!(recusa_da_linhagem(&aqui, "loja", &no(aqui.clone())), None);
    assert_eq!(
        recusa_da_linhagem(&aqui, "loja", &no(aqui.clone().com_linhagem(None))),
        None,
        "o source sem linhagem (v <= 10) foi recusado"
    );
    assert_eq!(
        recusa_da_linhagem(&aqui.clone().com_linhagem(None), "loja", &no(aqui)),
        None,
        "a tabela daqui sem linhagem (v <= 10) foi recusada"
    );
}
