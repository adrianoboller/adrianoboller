//! O PORTAO do `ao_alterar`, provado do unico jeito que serve: medindo.
//!
//! # Por que este arquivo existe, e por que sozinho
//!
//! O portao -- `alguma_coluna_indexada_mudou` -- e EXATO de proposito: tira-lo
//! nao muda resposta nenhuma, so faz toda alteracao pagar a varredura das
//! irmas. Entao ele nao tem prova semantica: escrevi uma, ela passou com o
//! defeito reposto, e teste que passa por engano e pior que teste que falta.
//!
//! O que ele tem e prova MEDIDA, e ela se calibra sozinha na maquina que
//! roda: a mesma tabela, a mesma linha, dois `atualizar` diferentes so no que
//! tocam. Medido nesta maquina, com 30 irmas: **21,7 us** para o que fica fora
//! do indice contra **1.179,6 us** para o que entra nele -- **54,4x**. O teste
//! cobra 5x, que e folga de dez vezes para maquina lenta, e ainda assim cai
//! para ~1x com o portao removido.
//!
//! Sozinho no arquivo porque `cargo test` roda os testes de um binario em
//! paralelo, e medir tempo com vizinho competindo pelo disco e medir o vizinho.

use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use std::time::Instant;

/// Quantas irmas a mae tem de perguntar quando o portao abre.
const IRMAS: usize = 30;
/// Voltas de cada medicao.
const VOLTAS: usize = 60;

#[test]
fn alterar_fora_do_indice_nao_paga_a_varredura_das_irmas() {
    let d = std::env::temp_dir().join(format!("phx-portao-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();

    // A mae tem tres colunas de proposito: `id` e referenciado pelas chaves,
    // `apelido` esta num indice e NAO e referenciado por ninguem, e `nome` nao
    // esta em indice nenhum. E o `apelido` que separa "o portao abriu" de "a
    // cascata aconteceu": alterar ele varre as irmas e nao cascateia nada.
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("apelido", ColumnType::Str(20)),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porApelido", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    let mut m = Table::criar(&d, e).unwrap();
    let r = m
        .inserir(&[
            Value::Int(1),
            Value::Str("a".into()),
            Value::Str("Ana".into()),
        ])
        .unwrap();
    m.sincronizar().unwrap();

    for i in 0..IRMAS {
        let e = Schema::new(
            format!("filha{i}"),
            vec![
                Column::new("id", ColumnType::Int4).obrigatoria(),
                Column::new("cliente_id", ColumnType::Int4),
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
        )
        .ao_alterar(AcaoRi::Cascata)])
        .unwrap();
        Table::criar(&d, e).unwrap();
    }

    // Aquece: a primeira volta paga abertura de arquivo que as outras nao pagam.
    for _ in 0..3 {
        m.atualizar(
            r,
            &[
                Value::Int(1),
                Value::Str("a".into()),
                Value::Str("x".into()),
            ],
        )
        .unwrap();
    }

    let t0 = Instant::now();
    for i in 0..VOLTAS {
        m.atualizar(
            r,
            &[
                Value::Int(1),
                Value::Str("a".into()),
                Value::Str(format!("n{i}")),
            ],
        )
        .unwrap();
    }
    let fora = t0.elapsed().as_secs_f64() / VOLTAS as f64;

    let t1 = Instant::now();
    for i in 0..VOLTAS {
        m.atualizar(
            r,
            &[
                Value::Int(1),
                Value::Str(format!("a{i}")),
                Value::Str("Ana".into()),
            ],
        )
        .unwrap();
    }
    let dentro = t1.elapsed().as_secs_f64() / VOLTAS as f64;

    println!(
        "fora do indice: {:.1} us/op | dentro do indice ({IRMAS} irmas): {:.1} us/op | {:.1}x",
        fora * 1e6,
        dentro * 1e6,
        dentro / fora
    );
    assert!(
        dentro > fora * 5.0,
        "o portao do `ao_alterar` caiu: alterar coluna fora de indice custou \
         {:.1} us e alterar coluna indexada custou {:.1} us -- so {:.1}x. Com o \
         portao no lugar sao dezenas de vezes, porque a varredura das {IRMAS} \
         irmas so acontece do lado de dentro",
        fora * 1e6,
        dentro * 1e6,
        dentro / fora
    );
}
