//! Chave composta: a LIVRE e a ÚNICA.
//!
//! As duas existem no formato desde o começo — `IndexDef` guarda uma lista de
//! colunas e um sinal de único —, mas nunca tinham teste que separasse os dois
//! casos. Separar importa porque a diferença não é de grau: a livre aceita a
//! combinação repetida, a única **recusa antes de gravar**, e recusar antes é
//! o que impede um slot fantasma no `.reg`, que não reaproveita espaço.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// `empresa` + `filial` + `documento`: a composta de três colunas.
fn esquema(unica: bool) -> Schema {
    let mut idx = IndexDef::new(
        "porDocumento",
        vec![
            IndexColumn::asc(0),
            IndexColumn::asc(1),
            IndexColumn::asc(2),
        ],
    );
    if unica {
        idx = idx.unico();
    }
    Schema::new(
        "notas",
        vec![
            Column::new("empresa", ColumnType::Int4).obrigatoria(),
            Column::new("filial", ColumnType::Int4).obrigatoria(),
            Column::new("documento", ColumnType::Str(20)).obrigatoria(),
            Column::new(
                "valor",
                ColumnType::Decimal {
                    precisao: 12,
                    escala: 2,
                },
            ),
        ],
        vec![idx],
    )
    .unwrap()
}

fn linha(e: i64, f: i64, doc: &str, v: i128) -> Vec<Value> {
    vec![
        Value::Int(e),
        Value::Int(f),
        Value::Str(doc.into()),
        Value::Decimal(v),
    ]
}

/// A composta LIVRE aceita a combinação repetida — é um índice de busca, não
/// uma restrição.
#[test]
fn composta_livre_aceita_repetida() {
    let dir = DirTemp::novo("composta-livre");
    let mut t = Table::criar(&dir.0, esquema(false)).unwrap();

    t.inserir(&linha(1, 1, "NF-001", 10_000)).unwrap();
    t.inserir(&linha(1, 1, "NF-001", 20_000))
        .expect("a composta livre recusou uma combinação repetida");
    t.inserir(&linha(1, 2, "NF-001", 30_000)).unwrap();

    assert_eq!(t.registros(), 3);
    // E busca pelas TRÊS colunas devolve as duas que batem.
    let achados = t
        .buscar(
            "porDocumento",
            &[Value::Int(1), Value::Int(1), Value::Str("NF-001".into())],
        )
        .unwrap();
    assert_eq!(achados.len(), 2, "a busca pela composta não achou as duas");
}

/// A composta ÚNICA recusa a combinação repetida — e recusa **antes** de
/// gravar, para não deixar slot morto.
#[test]
fn composta_unica_recusa_repetida_sem_deixar_buraco() {
    let dir = DirTemp::novo("composta-unica");
    let mut t = Table::criar(&dir.0, esquema(true)).unwrap();

    t.inserir(&linha(1, 1, "NF-001", 10_000)).unwrap();
    let slots_antes = t.slots();

    let erro = t
        .inserir(&linha(1, 1, "NF-001", 99_900))
        .expect_err("a composta única aceitou uma combinação repetida");
    assert!(
        erro.to_string().contains("porDocumento"),
        "a mensagem não diz qual índice recusou: {erro}"
    );

    // O ponto: a recusa não consumiu slot. Se consumisse, uma carga com muita
    // repetição iria inchando o arquivo sem nunca crescer a tabela.
    assert_eq!(t.slots(), slots_antes, "a recusa deixou um slot morto");
    assert_eq!(t.registros(), 1);

    // Mudar QUALQUER uma das três colunas já é outra chave.
    t.inserir(&linha(1, 2, "NF-001", 40_000)).unwrap();
    t.inserir(&linha(2, 1, "NF-001", 50_000)).unwrap();
    t.inserir(&linha(1, 1, "NF-002", 60_000)).unwrap();
    assert_eq!(t.registros(), 4);
}

/// A alteração respeita a composta única: mudar uma linha para uma combinação
/// que já é de outra é recusado; mudar para a dela mesma, não.
#[test]
fn alterar_respeita_a_composta_unica() {
    let dir = DirTemp::novo("composta-alterar");
    let mut t = Table::criar(&dir.0, esquema(true)).unwrap();
    t.inserir(&linha(1, 1, "NF-001", 10_000)).unwrap();
    t.inserir(&linha(1, 1, "NF-002", 20_000)).unwrap();

    // Levar a segunda para a chave da primeira: recusado.
    assert!(t.atualizar(2, &linha(1, 1, "NF-001", 20_000)).is_err());

    // Reescrever a segunda com a chave DELA mesma: aceito — senão salvar uma
    // ficha sem mexer na chave seria impossível.
    t.atualizar(2, &linha(1, 1, "NF-002", 77_700)).unwrap();
    assert_eq!(t.ler(2).unwrap().unwrap()[3], Value::Decimal(77_700));
}

/// O esquema declara que a chave é composta, e a tela lê isso dali — em vez de
/// contar colunas por conta própria.
#[test]
fn o_esquema_declara_que_a_chave_e_composta() {
    let dir = DirTemp::novo("composta-declara");
    let t = Table::criar(&dir.0, esquema(true)).unwrap();
    let i = t.esquema().indices().iter().find(|i| i.unico).unwrap();
    assert!(i.composta(), "três colunas e o índice não se diz composto");

    let simples = Schema::new(
        "s",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    assert!(!simples.indices()[0].composta());
}
