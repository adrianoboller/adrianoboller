//! **Pedido 422: a regravacao do esquema em duas fases.**
//!
//! Quando o bloco de esquema nao cabe antes do slot 1, a reescrita de cada
//! volume vira FASE A (`*.novo` completo e sincronizado, sem `rename`) e FASE
//! B (so os `rename`). O servidor roda a FASE A fora da trava global, com a
//! tabela congelada; estes testes provam o que o armazem garante sozinho:
//!
//! 1. o caminho feliz troca, e a linha de antes continua legivel;
//! 2. **escrita no volume entre as fases aborta a troca** -- o retrato nao
//!    bate, os `*.novo` saem, e a linha escrita no meio NAO some. E a guarda
//!    que segura o dado se um caminho novo escapar do congelamento amanha.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Um nome que estoura a folga do alinhamento: o bloco de esquema passa a
/// nao caber antes do slot 1, e a declaracao tem de reescrever o volume.
const NOME_COMPRIDO: &str =
    "fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento";

fn tabela(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    let mut t = Table::criar(d, e).unwrap();
    t.inserir(&[Value::Int(1), Value::Int(7)]).unwrap();
    t.sincronizar().unwrap();
    t
}

/// Sem conferir: a varredura nao e o assunto aqui, e a mae nem existe.
fn chaves() -> Vec<ForeignKey> {
    vec![ForeignKey::new(NOME_COMPRIDO, vec![1], "clientes", vec!["id".into()]).conferindo(false)]
}

fn novos(d: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(d)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".novo"))
        .collect()
}

#[test]
fn as_duas_fases_trocam_e_a_linha_de_antes_continua() {
    let d = DirTemp::novo("troca-esquema-feliz");
    let mut t = tabela(&d.0);
    let fks = chaves();
    let troca = t
        .preparar_chaves_estrangeiras(&fks)
        .unwrap()
        .expect("o preparo falhou: o nome nao forcou a reescrita");
    assert!(!novos(&d.0).is_empty(), "a FASE A nao deixou `*.novo`");
    let recibo = t.conferir_chaves_que_nascem(&fks, None).unwrap();
    assert!(t
        .redeclarar_depois_de_conferir(fks, recibo, Some(troca))
        .unwrap());
    assert!(novos(&d.0).is_empty(), "sobrou `*.novo`: {:?}", novos(&d.0));
    drop(t);

    let mut t = Table::abrir(&d.0, "pedidos").unwrap();
    assert_eq!(t.esquema().chaves_estrangeiras()[0].nome, NOME_COMPRIDO);
    let l = t.ler(1).unwrap().expect("a linha de antes sumiu");
    assert_eq!(l[1], Value::Int(7));
}

#[test]
fn escrita_entre_as_fases_aborta_a_troca_e_nao_perde_a_linha() {
    let d = DirTemp::novo("troca-esquema-retrato");
    let mut t = tabela(&d.0);
    let fks = chaves();
    let troca = t
        .preparar_chaves_estrangeiras(&fks)
        .unwrap()
        .expect("o preparo falhou: o nome nao forcou a reescrita");

    // A escrita que o congelamento existe para impedir, feita por baixo dele.
    t.inserir(&[Value::Int(2), Value::Int(8)]).unwrap();
    t.sincronizar().unwrap();

    let recibo = t.conferir_chaves_que_nascem(&fks, None).unwrap();
    let e = t
        .redeclarar_depois_de_conferir(fks, recibo, Some(troca))
        .expect_err("a troca renomeou por cima de escrita confirmada");
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(
        novos(&d.0).is_empty(),
        "o aborto deixou `*.novo`: {:?}",
        novos(&d.0)
    );
    drop(t);

    let mut t = Table::abrir(&d.0, "pedidos").unwrap();
    assert!(
        t.esquema().chaves_estrangeiras().is_empty(),
        "a troca abortada gravou o esquema novo"
    );
    let l = t
        .ler(2)
        .unwrap()
        .expect("a linha escrita entre as fases SUMIU");
    assert_eq!(l[1], Value::Int(8));
    assert!(t.ler(1).unwrap().is_some(), "a linha de antes sumiu");
}
