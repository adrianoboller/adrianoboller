//! A exclusao fisica dentro da janela de durabilidade -- **quando o dono pede**.
//!
//! # Por que isto e um teste de INTEGRACAO
//!
//! Pela mesma razao do corte do diario (`tests/corte-do-diario.rs`): o
//! interruptor e um global do PROCESSO, e `cargo test` roda os testes do mesmo
//! binario em paralelo. Um teste que ligasse a janela dentro do binario da
//! `--lib` tiraria o `fsync` da exclusao de todos os outros que estao correndo
//! ao lado -- inclusive o que existe justamente para provar que ele esta la.
//!
//! O teste do comportamento VELHO mora do lado de la, em `tests/exclusao.rs`
//! (`sem_pedir_a_janela_cada_exclusao_espera_o_disco`), e e ele o que mais
//! importa nesta frente: quem nao pediu nada continua com a garantia de
//! sempre.
//!
//! # O que se perde na janela, e o que este arquivo prova
//!
//! Com a janela ligada ha um intervalo entre o `excluir` responder e o
//! `.trash` estar no disco. Nesse intervalo:
//!
//! - **queda do PROCESSO** (`kill -9`, panico, o servidor sendo reiniciado):
//!   nao se perde nada. O `write` ja esta no sistema operacional, e quem
//!   reabre le a mesma pagina. E o que `a_linha_esta_no_trash_mesmo_sem_fsync`
//!   prova aqui, e o que `bancada/exclusao/prova-da-queda.py` prova de novo
//!   com um `phxsqld` de verdade morrendo a `kill -9` -- porque teste unitario
//!   nao prova queda de processo, processo prova.
//! - **queda de ENERGIA**: perde-se o que entrou na janela. Ver
//!   `docs/DESEMPENHO.md` §4.12 para o caso a caso, inclusive o unico em que a
//!   linha some dos dois lugares que se leem.
//!
//! A ordem de ESCRITA nao muda em nenhum dos dois modos, e e isso que
//! `a_linha_esta_no_trash_mesmo_sem_fsync` trava.

mod comum;
use std::sync::Mutex;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::lixeira;
use phxsql_store::table::Table;

/// O global e um so: dois testes ligando e desligando ao mesmo tempo mediriam
/// o modo do vizinho.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// Liga a janela e a desliga na saida, doa o que der -- inclusive num
/// `assert!` que estoure no meio. Sem isto, um teste que falha deixaria o
/// global ligado para o proximo, e o proximo mediria outra coisa.
struct ComAJanela;

impl ComAJanela {
    fn nova() -> ComAJanela {
        lixeira::definir_na_janela(true);
        ComAJanela
    }
}

impl Drop for ComAJanela {
    fn drop(&mut self) {
        lixeira::definir_na_janela(false);
    }
}

fn dir(rotulo: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("exc-janela-{rotulo}"))
}

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn linha(id: i64) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(format!("Cliente {id}")),
        Value::Memo(format!("ficha longa do cliente {id}, com texto de sobra")),
    ]
}

fn com_dados(d: &std::path::Path, quantas: i64) -> Table {
    let mut t = Table::criar(d, esquema()).unwrap();
    for i in 1..=quantas {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    t
}

/// **O ganho:** pedida a janela, o `fsync` sai do caminho de quem exclui.
///
/// Reponha o defeito tirando o `if !na_janela()` de `LixeiraFile::guardar` --
/// o campo passa a nao ser lido, que e a armadilha do `cache_paginas` sem
/// cache -- e este teste cai na primeira assercao.
#[test]
fn pedida_a_janela_o_fsync_sai_do_caminho() {
    let _so_eu = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("sai-do-caminho");
    let mut t = com_dados(&d, 40);

    let _janela = ComAJanela::nova();
    let antes = t.lixeira_sincronizacoes();
    for rowid in 1..=20 {
        assert!(t.excluir_de_vez(rowid, "na janela").unwrap());
    }
    assert_eq!(
        t.lixeira_sincronizacoes(),
        antes,
        "vinte exclusoes na janela e o .trash ainda esperou o disco"
    );

    // ... e a janela fecha com a tabela, uma vez por todas.
    t.sincronizar().unwrap();
    assert_eq!(t.lixeira_sincronizacoes(), antes + 1);
}

/// A ORDEM DE ESCRITA nao muda: guardar antes de liberar, nos dois modos.
///
/// O que a janela troca e quem espera o disco. Se alguem trocar a ordem para
/// "libera e depois guarda" -- a unica forma de fazer a exclusao render mais
/// do que isto --, o `.reg` sai na frente e a linha some dos dois lados por
/// construcao, e nao por azar. Este teste mata a tabela sem sincronizar nada
/// e cobra as duas metades.
#[test]
fn a_linha_esta_no_trash_mesmo_sem_fsync() {
    let _so_eu = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("sem-fsync");
    {
        let mut t = com_dados(&d, 10);
        let _janela = ComAJanela::nova();
        for rowid in [2u64, 4, 6] {
            assert!(t.excluir_de_vez(rowid, "sem sincronizar depois").unwrap());
        }
        // De proposito: NAO sincroniza. E a queda do PROCESSO -- o `write` ja
        // foi entregue ao sistema operacional, e quem reabre le a mesma
        // pagina. O que o `fsync` protege e a queda de ENERGIA, que teste
        // nenhum provoca.
    }

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let lixo = t.lixeira(0, 0, true).unwrap();
    assert_eq!(lixo.len(), 3, "a lixeira perdeu linha na queda do processo");
    let mut rowids: Vec<u64> = lixo.iter().map(|l| l.rowid).collect();
    rowids.sort_unstable();
    assert_eq!(rowids, vec![2, 4, 6]);
    // Inteira, com o Memo junto: a lixeira guarda o CONTEUDO do externo, e o
    // bloco dele ja foi liberado.
    assert_eq!(
        t.linha_da_lixeira(&lixo[0]).unwrap()[1],
        Value::Str("Cliente 2".into())
    );
    // E do outro lado: nenhuma das tres sobrou no `.reg`.
    for rowid in [2u64, 4, 6] {
        assert!(t.ler(rowid).unwrap().is_none(), "o rowid {rowid} nao saiu");
    }
    assert!(
        t.ler(3).unwrap().is_some(),
        "levou junto quem nao foi pedido"
    );
}

/// O interruptor volta sozinho: desligado, a exclusao seguinte ja espera o
/// disco de novo.
///
/// Importa porque o campo e editavel a quente (`CAMPOS_EDITAVEIS`): quem
/// desligar pela tela tem de recuperar a garantia na operacao seguinte, e nao
/// no proximo arranque.
#[test]
fn desligar_devolve_a_garantia_na_hora() {
    let _so_eu = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir("vai-e-volta");
    let mut t = com_dados(&d, 10);

    {
        let _janela = ComAJanela::nova();
        let antes = t.lixeira_sincronizacoes();
        t.excluir_de_vez(1, "na janela").unwrap();
        assert_eq!(t.lixeira_sincronizacoes(), antes);
    }
    let antes = t.lixeira_sincronizacoes();
    t.excluir_de_vez(2, "de volta ao de sempre").unwrap();
    assert_eq!(
        t.lixeira_sincronizacoes(),
        antes + 1,
        "desligar a janela nao devolveu o fsync por exclusao"
    );
}
