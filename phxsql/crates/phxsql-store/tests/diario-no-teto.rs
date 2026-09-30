//! **Pedido 498: o diario no teto de volumes recusa ANTES de a linha ir ao
//! disco.** Antes, a alteracao gravava o valor novo no `.reg` e so entao o
//! `.log` recusava: linha sem evento -- replica divergindo, cascata pulada.
//!
//! Tabela paginada com 3 volumes de 2 KiB: o diario enche depois de algumas
//! dezenas de eventos. A partir dai toda escrita tem de recusar SEM mudar o
//! que esta no `.reg`.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::error::PhxError;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn tabela(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "contas",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("email", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_paginacao(
        Paginacao::nova(100_000, 3)
            .unwrap()
            .com_bytes_por_arquivo(2048)
            .unwrap(),
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

#[test]
fn no_teto_do_diario_a_alteracao_recusa_sem_gravar_a_linha() {
    let d = DirTemp::novo("diario-no-teto");
    let mut t = tabela(&d.0);
    let rowid = t
        .inserir(&[Value::Int(1), Value::Str("a0@x.com".into())])
        .unwrap();
    let mut ultimo_ok = "a0@x.com".to_string();
    let mut recusa = None;
    for i in 1..=2000 {
        let email = format!("a{i}@x.com");
        match t.atualizar(rowid, &[Value::Int(1), Value::Str(email.clone())]) {
            Ok(()) => ultimo_ok = email,
            Err(e) => {
                recusa = Some(e);
                break;
            }
        }
    }
    let e = recusa.expect("o diario nunca chegou ao teto -- o teste nao mediu nada");
    assert!(
        matches!(&e, PhxError::LimiteExcedido(m) if m.contains("diario")),
        "recusou por outro motivo: {e:?}"
    );
    let linha = t.ler(rowid).unwrap().unwrap();
    assert_eq!(
        linha[1],
        Value::Str(ultimo_ok.clone()),
        "a alteracao recusada GRAVOU o valor novo no .reg"
    );
    // E a insercao, no mesmo teto, tambem recusa sem deixar linha.
    let antes = t.registros();
    assert!(t
        .inserir(&[Value::Int(2), Value::Str("b@x.com".into())])
        .is_err());
    assert_eq!(
        t.registros(),
        antes,
        "a insercao recusada deixou linha no .reg"
    );
}
