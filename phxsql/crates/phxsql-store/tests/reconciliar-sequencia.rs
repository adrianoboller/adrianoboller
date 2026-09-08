//! Defeito (c) do pedido 229 -- o pedaco LOCAL: um contador de `Sequence`
//! atras do dado repete numero, calado.
//!
//! O bloco 16 da sonda mediu: `ajustar_sequencia` para tras numa tabela SEM
//! indice unico devolve ids `[1, 2, 3, 1]` -- quatro linhas, um id repetido,
//! nenhum erro. O CRC-32 do cabecalho nao pega isso, porque `ajustar_sequencia`
//! reescreve o cabecalho com CRC valido; so pega bytes adulterados (bloco 17).
//!
//! `reconciliar_sequencia` e o caminho de reparo que faltava, e `reparar` passou
//! a chama-lo. Prova real nos dois sentidos: sem a reconciliacao o id repete
//! (o defeito reposto), com ela o contador volta para depois do maior gravado.
//!
//! A promocao de uma replica ATRASADA e outra coisa, e nao se conserta aqui:
//! os numeros que o master emitiu e esta ponta nunca recebeu nao estao neste
//! `.reg`. Isso esta documentado em `docs/AUTONUMBER.md`, defeito (c).

mod comum;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema_sem_unico() -> Schema {
    // SEM indice unico sobre o id, de proposito: e o unico jeito de o repeat
    // acontecer calado. Com indice unico, o proprio indice recusaria (bloco 6).
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Sequence).obrigatoria(),
            Column::new("c", ColumnType::Str(30)).obrigatoria(),
        ],
        vec![IndexDef::new("porC", vec![IndexColumn::asc(1)])],
    )
    .expect("esquema com Sequence sem indice unico")
}

fn id_da_linha(t: &mut Table, rowid: u64) -> u64 {
    match &t.ler(rowid).unwrap().unwrap()[0] {
        Value::UInt(n) => *n,
        outro => panic!("id nao e UInt: {outro:?}"),
    }
}

/// O DEFEITO, reposto: sem reconciliar, ajustar para tras faz o id repetir.
/// Este teste documenta o bloco 16 -- ele passa hoje e continua passando,
/// porque descreve o que acontece QUANDO NAO se reconcilia.
#[test]
fn contador_atras_do_dado_repete_o_numero_calado() {
    let d = comum::DirTemp::novo("reconciliar-repete");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();

    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    // Contador do administrador para tras, abaixo do maior gravado (3).
    t.ajustar_sequencia(1).unwrap();

    let rowid = t
        .inserir(&[Value::Null, Value::Str("repetido".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, rowid),
        1,
        "sem reconciliar, o contador atrasado reemite o id 1 -- e o defeito"
    );
}

/// O CONSERTO: reconciliar empurra o contador para depois do maior gravado, e
/// a proxima insercao nao repete.
///
/// Reponha o defeito trocando o `t.reconciliar_sequencia()` por nada: a
/// insercao volta a sair com id 1 e a ultima asserção cai.
#[test]
fn reconciliar_empurra_o_contador_para_depois_do_maior() {
    let d = comum::DirTemp::novo("reconciliar-conserta");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();

    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    t.ajustar_sequencia(1).unwrap();

    // O reparo do contador: devolve o maior valor gravado (3).
    let maior = t.reconciliar_sequencia().unwrap();
    assert_eq!(maior, 3, "o maior id gravado e 3");

    let rowid = t
        .inserir(&[Value::Null, Value::Str("depois".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, rowid),
        4,
        "reconciliado, o proximo id e maior+1 = 4, e nao repete"
    );
}

/// Reconciliar so empurra para a FRENTE: um contador ja adiantado (por buraco
/// de exclusao fisica, por exemplo) fica onde esta -- o numero excluido nunca
/// volta, e reconciliar nao o traz de volta.
#[test]
fn reconciliar_nunca_recua_o_contador() {
    let d = comum::DirTemp::novo("reconciliar-nao-recua");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();

    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    // Contador bem a frente do dado, de propria vontade do administrador.
    t.ajustar_sequencia(1000).unwrap();

    let maior = t.reconciliar_sequencia().unwrap();
    assert_eq!(maior, 3);

    let rowid = t
        .inserir(&[Value::Null, Value::Str("depois".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, rowid),
        1000,
        "reconciliar nao puxa o contador para tras -- o 1000 do administrador fica"
    );
}

/// Tabela sem coluna `Sequence`: reconciliar e no-op, devolve 0 e nao explode.
#[test]
fn sem_sequencia_reconciliar_e_zero() {
    let d = comum::DirTemp::novo("reconciliar-sem-seq");
    let esquema = Schema::new(
        "notas",
        vec![Column::new("c", ColumnType::Str(30)).obrigatoria()],
        vec![],
    )
    .unwrap();
    let mut t = Table::criar(&d, esquema).unwrap();
    t.inserir(&[Value::Str("x".into())]).unwrap();
    assert_eq!(t.reconciliar_sequencia().unwrap(), 0);
}
