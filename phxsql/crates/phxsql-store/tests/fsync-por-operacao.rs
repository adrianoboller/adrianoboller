//! Guarda (pedido 258): em `por_operacao`, o fecho de uma operacao leva ao
//! disco SO' os arquivos que ela escreveu -- e uma tabela reaberta no mesmo
//! processo herda isso, porque o servidor abre uma `Table` por pedido.
//!
//! # O defeito que ela reprova
//!
//! `Volumes::sincronizar` sincronizava todo descritor aberto sem perguntar
//! quem mudou. Medido pelo nucleo (`--example fsync-por-operacao`, 16/09/2026):
//! num inserir, 4 dos 8 `fsync` iam a arquivo limpo (`.trash .bin .memo
//! .reason`); num atualizar, 5 de 8; num excluir, 3 de 9. Era isso, e nao a
//! arvore, que punha o padrao atras do SQLite em toda escrita com `fsync`
//! (bancada CRUD, pedido 257: 0,66x-0,94x).
//!
//! # O que ela conta, e por que isso e' o FATO
//!
//! `Table::arquivos_sincronizados()` soma o `Volumes::sincronizados()` das
//! sete familias, e esse contador so' sobe DEPOIS de o `sync_all` confirmar
//! -- e' o irmao em RAM do `strace`, valido dentro de um processo. O `.ndx`
//! fica fora da conta porque fica fora do conserto: ele nao passa pelo
//! `Volumes`, e o byte de sujo do cabecalho dele responde outra pergunta
//! (ver o parecer do DBA no pedido 258 e o `FORMATO.md` §8).
//!
//! **Prova real:** reponha o comportamento antigo em `sincronizar_listas`
//! (`alvos = abertos ∪ escritos`, sem o filtro dos batizados) e os tres
//! numeros sobem para 6, 6 e 7.

mod comum;

use comum::DirTemp;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "config",
        vec![
            Column::new("chave", ColumnType::Str(64)).obrigatoria(),
            Column::new("valor", ColumnType::Str(128)).obrigatoria(),
        ],
        vec![IndexDef::new("porChave", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(i: i64, versao: u32) -> Vec<Value> {
    vec![
        Value::Str(format!("g{}/s{}/v{i}", i % 7, i % 3)),
        Value::Str(format!("valor {i} v{versao}")),
    ]
}

/// A tabela semeada e ja' batizada: o primeiro fecho do processo pagou tudo.
fn semeada(d: &DirTemp) -> Table {
    // O padrao de fabrica: a lixeira espera o disco por exclusao.
    phxsql_store::lixeira::definir_na_janela(false);
    let mut t = Table::criar(d, esquema()).unwrap();
    for i in 1..=50 {
        t.inserir(&linha(i, 0)).unwrap();
    }
    t.sincronizar().unwrap();
    t
}

#[test]
fn depois_do_primeiro_fecho_so_o_que_a_operacao_escreveu_vai_ao_disco() {
    let d = DirTemp::novo("fsync-por-op");
    let mut t = semeada(&d);

    let antes = t.arquivos_sincronizados();
    t.inserir(&linha(51, 0)).unwrap();
    t.sincronizar().unwrap();
    assert_eq!(
        t.arquivos_sincronizados() - antes,
        2,
        "inserir escreve o `.reg` e o `.log`; `.trash .bin .memo .reason` estao          limpos e batizados, e nao pagam"
    );

    let antes = t.arquivos_sincronizados();
    t.atualizar(1, &linha(1, 1)).unwrap();
    t.sincronizar().unwrap();
    assert_eq!(
        t.arquivos_sincronizados() - antes,
        2,
        "atualizar regrava a linha no `.reg` e registra no `.log`"
    );

    let antes = t.arquivos_sincronizados();
    assert!(t.excluir(2).unwrap());
    t.sincronizar().unwrap();
    assert_eq!(
        t.arquivos_sincronizados() - antes,
        4,
        "excluir: o `.trash` no proprio `guardar` (fabrica), depois `.reason`,          `.log` e `.reg` no fecho -- o segundo `fsync` do `.trash`, que ia a          arquivo limpo, e' pulado"
    );
}

/// O caminho do SERVIDOR: `gravar_de_verdade` sincroniza a `Table` que
/// escreveu, e ela e' aberta por pedido. O que faz o pedido seguinte nao
/// pagar de novo os quatro limpos e' o registro ser do PROCESSO, e nao da
/// instancia -- um sinalizador dentro do `Table` (a forma que a §16.2 do
/// `DESEMPENHO.md` recusou) daria 6 aqui.
#[test]
fn a_tabela_reaberta_no_mesmo_processo_herda_o_batismo() {
    let d = DirTemp::novo("fsync-por-op-reaberta");
    drop(semeada(&d));

    let mut t = Table::abrir(&d, "config").unwrap();
    let antes = t.arquivos_sincronizados();
    t.inserir(&linha(51, 0)).unwrap();
    t.sincronizar().unwrap();
    assert_eq!(
        t.arquivos_sincronizados() - antes,
        2,
        "a instancia nova nunca escreveu nos quatro limpos, e o processo ja os          batizou: so' `.reg` e `.log` vao ao disco"
    );
}

/// O lado que NAO pode mudar: o primeiro fecho de uma familia no processo
/// leva tudo o que esta aberto, porque um processo anterior pode ter deixado
/// pagina suja no nucleo sem marca nenhuma em RAM. Aqui a familia e' nova
/// (diretorio novo), entao o registro dela esta vazio de verdade.
#[test]
fn o_primeiro_fecho_de_uma_familia_nova_leva_tudo_o_que_esta_aberto() {
    let d = DirTemp::novo("fsync-por-op-primeiro");
    phxsql_store::lixeira::definir_na_janela(false);
    let mut t = Table::criar(&d, esquema()).unwrap();
    t.inserir(&linha(1, 0)).unwrap();
    t.sincronizar().unwrap();
    // `.reg .bin .memo .log .trash .reason`: os seis que `criar` faz nascer
    // (o `.lgpd` e' preguicoso e ainda nao existe).
    assert_eq!(
        t.arquivos_sincronizados(),
        6,
        "o primeiro fecho do processo nao pula ninguem"
    );
}
