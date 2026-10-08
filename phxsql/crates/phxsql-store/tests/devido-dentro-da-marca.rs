//! **Pedido 715 (F6): o evento devido que a recuperacao completa leva o id da
//! transacao dele, e nao um novo.**
//!
//! A passada de um `COMMIT` grava a venda e um item; o `.log` do segundo item
//! falha (pedido 498: a linha no `.reg`, o evento devido no cabecalho) e o
//! processo cai. No arranque, a recuperacao abre a unidade da marca e abre as
//! tabelas -- e a abertura completa o devido, que tirava um id NOVO da unidade
//! antes de ela adotar o da marca. A cauda passava a ter dois ids, a adocao
//! falhava, e o resto ia com o novo: **a transacao partia em duas na replica
//! encadeada**.
//!
//! A falha e forjada (`sincronia::falha_de_teste`), e por isso so existe com
//! `debug_assertions`. Arquivo proprio porque esquece o contador de ids do
//! processo (o «processo novo» do arranque), e isso nao pode correr ao lado
//! de outro teste.

#![cfg(debug_assertions)]

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::PoliticaDoDiario;
use phxsql_store::marca::{
    gravar_marca_posicional, recuperar_no_diretorio, Acao, Bilhete, Escrita,
};
use phxsql_store::sincronia::falha_de_teste::{armar, desarmar, Onde};
use phxsql_store::table::Table;

fn tabela(d: &std::path::Path, nome: &str) -> Table {
    let e = Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("venda", ColumnType::Int4),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(d, e).unwrap();
    t.ligar_imagem_no_diario(true);
    t
}

fn inclusao(tabela: &str, rowid: u64, id: i64) -> Escrita {
    Escrita {
        database: String::new(),
        tabela: tabela.into(),
        acao: Acao::Inserir,
        rowid,
        linha: vec![Value::Int(id), Value::Int(1)],
        linha_antiga: Vec::new(),
        motivo: String::new(),
        cascata_na_lista: false,
        elo_do_empilhar: false,
        elo_da_cascata: false,
    }
}

fn ids(d: &std::path::Path, nome: &str) -> Vec<u64> {
    let mut t = Table::abrir(d, nome).unwrap();
    let total = t.eventos().unwrap();
    t.diario(0, total).unwrap().iter().map(|e| e.tx).collect()
}

#[test]
fn o_devido_da_passada_completa_com_o_id_da_marca() {
    let d = DirTemp::novo("devido-na-marca");
    let mut vendas = tabela(&d.0, "vendas");
    let mut itens = tabela(&d.0, "itens");
    let ops = vec![
        inclusao("vendas", 1, 1),
        inclusao("itens", 1, 1),
        inclusao("itens", 2, 2),
    ];

    // O «servidor»: a tomada abre a unidade, o COMMIT reserva o id e grava a
    // marca, e a passada comeca.
    phxsql_store::log::abrir_unidade();
    let tx = phxsql_store::log::reservar_tx_na_unidade();
    gravar_marca_posicional(
        &d.0,
        7,
        1,
        &ops,
        Bilhete {
            tx,
            versoes_antes: &[0, 0, 0],
        },
    )
    .unwrap();
    vendas.inserir(&ops[0].linha).unwrap();
    armar(&d.0, Onde::GravacaoDoDiario, 1);
    let e = itens.inserir(&ops[1].linha).unwrap_err();
    desarmar(&d.0);
    assert!(e.to_string().contains("pedido 498"), "{e}");
    phxsql_store::log::fechar_unidade();
    drop(vendas);
    drop(itens);

    // O arranque: processo novo, contador do zero.
    phxsql_store::log::esquecer_ultimo_tx_para_teste();
    let r = recuperar_no_diretorio(&d.0, PoliticaDoDiario::com_imagem(true));
    assert!(r.impossiveis.is_empty(), "{:?}", r.impossiveis);

    let mut todos = ids(&d.0, "vendas");
    todos.extend(ids(&d.0, "itens"));
    assert_eq!(todos.len(), 3, "a venda nao ficou inteira no diario");
    assert!(
        todos.iter().all(|&t| t == tx),
        "a transacao {tx} saiu partida no diario: {todos:?}"
    );
}
