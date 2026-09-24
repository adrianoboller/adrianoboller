//! NULL num indice UNICO nao colide -- pedido 448, achado A4 da revisao do
//! DBA (`docs/propostas/parecer-dba-448-2026-09-24.md`).
//!
//! PostgreSQL, MySQL, MariaDB e SQLite aceitam varios NULL num `UNIQUE`: NULL
//! nao e igual a nada, nem a outro NULL. Aceite automatico. Aqui a regra
//! estava escrita duas vezes e as duas divergiam -- o store dava DUPLICADO no
//! segundo NULL, e a conferencia da transacao no servidor pulava o NULL --, e
//! a divergencia saia como «DEFEITO DO MOTOR» no COMMIT. Hoje a pergunta mora
//! num lugar so (`Table::participa_da_unicidade`), e esta prova a exercita por
//! todas as portas do store: o `inserir`, o `atualizar`, o `reindexar` e a
//! conferencia da transacao.

mod comum;
use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn dir(nome: &str) -> comum::DirTemp {
    comum::DirTemp::novo(&format!("nulo-unico-{nome}"))
}

/// `clientes(id, email)` com `email` UNICO e anulavel, e um unico COMPOSTO
/// `(id, email)` para a regra valer por componente.
fn clientes(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("email", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porEmail", vec![IndexColumn::asc(1)]).unico(),
            IndexDef::new("porEmailDesc", vec![IndexColumn::desc(1)]).unico(),
        ],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// Prova real: devolver a conferencia do `inserir` a `unico && existe` (sem a
/// regra do NULL) faz o segundo NULL cair em DUPLICADO; devolver a do
/// `atualizar` faz a volta a NULL cair; e tirar o predicado do `reindexar`
/// faz a reconstrucao recusar a tabela que o `inserir` aceitou.
#[test]
fn varios_nulos_num_indice_unico_passam_por_todas_as_portas() {
    let d = dir("portas");
    let mut t = clientes(&d);
    t.inserir(&[Value::Int(1), Value::Null])
        .expect("o primeiro NULL");
    t.inserir(&[Value::Int(2), Value::Null])
        .expect("o segundo NULL num indice unico tinha de passar");
    let r3 = t.inserir(&[Value::Int(3), Value::Str("x".into())]).unwrap();
    // O `atualizar` que VOLTA a NULL, com dois NULL ja gravados.
    t.atualizar(r3, &[Value::Int(3), Value::Null])
        .expect("voltar a NULL com outros NULL no indice tinha de passar");
    // A conferencia da transacao, a mesma pergunta.
    t.conferir_unicidade(&[Value::Int(4), Value::Null], None)
        .expect("a conferencia da transacao tinha de deixar o NULL passar");
    t.sincronizar().unwrap();
    // A reconstrucao do indice, com tres NULL no lote.
    t.reindexar()
        .expect("o reindexar recusou a tabela que o inserir aceitou");
    assert_eq!(t.registros(), 3);

    // E o que NAO muda: o valor repetido continua recusado, nas tres portas.
    t.atualizar(r3, &[Value::Int(3), Value::Str("y".into())])
        .unwrap();
    let e = t
        .inserir(&[Value::Int(5), Value::Str("y".into())])
        .unwrap_err();
    assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
    let e = t
        .conferir_unicidade(&[Value::Int(6), Value::Str("y".into())], None)
        .unwrap_err();
    assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
    let e = t
        .atualizar(1, &[Value::Int(1), Value::Str("y".into())])
        .unwrap_err();
    assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
}
