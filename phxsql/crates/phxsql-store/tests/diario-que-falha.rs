//! **Pedido 498, o resto: o `.log` que falha DEPOIS de a linha estar no
//! `.reg`.** O teto de volumes ja recusa antes (`diario-no-teto.rs`); o disco
//! que enche entre as duas gravacoes, nao. O catalogo do 496 mediu 294 linhas
//! sem evento em 22 de 41 tamanhos de tmpfs, e uma orfa.
//!
//! Decisao do dono (30/09/2026): derrubar e completar, como o PANIC do
//! PostgreSQL na falha de escrita do WAL. A queda do servidor se prova no
//! `phxsql-server` (o gancho e dele); aqui se prova o que a BIBLIOTECA faz,
//! que nao cai: a marca do evento devido fica no cabecalho do `.log`, a
//! tabela recusa toda escrita ate ser reaberta, e a abertura completa o
//! evento pela linha -- com a imagem, a versao, o carimbo e a origem certos.
//!
//! A falha e forjada (`sincronia::falha_de_teste`, ENOSPC no `write` do
//! evento), e por isso so existe com `debug_assertions`. A prova contra o
//! disco cheio de verdade esta na bancada (`bancada/catastrofes/`).

#![cfg(debug_assertions)]

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
use phxsql_store::sincronia::falha_de_teste::{armar, desarmar, Onde};
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "contas",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("email", ColumnType::Str(40)),
            Column::new("obs", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(id: i32, email: &str) -> Vec<Value> {
    vec![
        Value::Int(id as i64),
        Value::Str(email.into()),
        Value::Memo(format!("anexo de {email}")),
    ]
}

fn criar(d: &std::path::Path) -> Table {
    let mut t = Table::criar(d, esquema()).unwrap();
    t.ligar_imagem_no_diario(true);
    t
}

fn abrir(d: &std::path::Path) -> Table {
    let mut t = Table::abrir(d, "contas").unwrap();
    t.ligar_imagem_no_diario(true);
    t
}

/// A marca de pe no cabecalho do volume 1 do `.log` (bytes 40..56 na versao
/// 2): o primeiro byte e a operacao (o bit 7 diz «com imagem»), e zero quer
/// dizer «nada devido».
fn marca_no_disco(d: &std::path::Path) -> u8 {
    let bruto = std::fs::read(d.join("contas.log")).unwrap();
    bruto[40] & 0x7f
}

#[test]
fn a_insercao_cujo_diario_falha_completa_na_abertura() {
    let d = DirTemp::novo("diario-falha-inclusao");
    let mut t = criar(&d.0);
    t.inserir(&linha(1, "a@x.com")).unwrap();
    let antes = t.eventos().unwrap();

    armar(&d.0, Onde::GravacaoDoDiario, 1);
    let e = t.inserir(&linha(2, "b@x.com")).unwrap_err();
    desarmar(&d.0);
    assert!(e.to_string().contains("pedido 498"), "{e}");
    // A linha esta no `.reg` -- e isso que a queda do servidor protege.
    let rowid = 2;
    assert!(
        t.ler(rowid).unwrap().is_some(),
        "a linha nao chegou ao .reg"
    );
    assert_eq!(
        t.eventos().unwrap(),
        antes,
        "o evento entrou mesmo com a falha"
    );
    assert_eq!(
        marca_no_disco(&d.0),
        1,
        "a marca do evento devido nao ficou"
    );

    // Devendo, a tabela recusa escrever: um evento depois do devido poria na
    // replica a alteracao de uma linha que ela ainda nao recebeu.
    let vivas = t.registros();
    let e = t.inserir(&linha(3, "c@x.com")).unwrap_err();
    assert!(e.to_string().contains("deve o evento"), "{e}");
    assert_eq!(t.registros(), vivas, "a escrita recusada gravou a linha");
    drop(t);

    // A abertura completa.
    let mut t = abrir(&d.0);
    assert_eq!(
        t.eventos().unwrap(),
        antes + 1,
        "a abertura nao completou o evento devido: a linha {rowid} ficou sem diario"
    );
    let (ev, imagem) = t.diario_com_imagem(antes, 1).unwrap().pop().unwrap();
    assert_eq!(
        (ev.operacao, ev.rowid, ev.versao),
        (Operacao::Inclusao, rowid, 1)
    );
    assert_eq!(
        imagem,
        t.imagem_da_linha_do_rowid(rowid).unwrap(),
        "a imagem completada nao e a da linha"
    );
    assert_eq!(marca_no_disco(&d.0), 0, "a marca continuou de pe");
    // E a tabela volta a gravar, com o evento na ordem.
    t.inserir(&linha(3, "c@x.com")).unwrap();
    assert_eq!(t.eventos().unwrap(), antes + 2);
    drop(t);
    // Reabrir de novo nao duplica nada.
    let mut t = abrir(&d.0);
    assert_eq!(
        t.eventos().unwrap(),
        antes + 2,
        "a segunda abertura duplicou"
    );
}

#[test]
fn a_alteracao_e_a_exclusao_completam_com_a_versao_e_a_imagem_certas() {
    let d = DirTemp::novo("diario-falha-alteracao");
    let mut t = criar(&d.0);
    t.ligar_imagem_na_exclusao(true);
    for i in 1..=3 {
        t.inserir(&linha(i, &format!("x{i}@x.com"))).unwrap();
    }
    // Alteracao.
    let antes = t.eventos().unwrap();
    armar(&d.0, Onde::GravacaoDoDiario, 1);
    assert!(t.atualizar(2, &linha(2, "novo@x.com")).is_err());
    desarmar(&d.0);
    drop(t);
    let mut t = abrir(&d.0);
    t.ligar_imagem_na_exclusao(true);
    let (ev, imagem) = t.diario_com_imagem(antes, 0).unwrap().pop().unwrap();
    assert_eq!((ev.operacao, ev.rowid), (Operacao::Alteracao, 2));
    assert_eq!(ev.versao, 2, "a versao completada nao e a do .reg");
    let (payload, externos) = Table::abrir_imagem(&imagem).unwrap();
    assert_eq!(imagem, t.imagem_da_linha_do_rowid(2).unwrap());
    assert!(!payload.is_empty());
    assert_eq!(externos[0].1, b"anexo de novo@x.com");

    // Exclusao de vez: a linha sai do `.reg`, e a imagem do evento sai da
    // lixeira, que a guardou com o conteudo do `.memo`.
    let antes = t.eventos().unwrap();
    armar(&d.0, Onde::GravacaoDoDiario, 1);
    assert!(t.excluir_de_vez(3, "teste").is_err());
    desarmar(&d.0);
    assert!(t.ler(3).unwrap().is_none());
    drop(t);
    let mut t = abrir(&d.0);
    let eventos = t.diario_com_imagem(antes, 0).unwrap();
    assert_eq!(eventos.len(), 1, "a exclusao ficou sem evento");
    let (ev, imagem) = &eventos[0];
    assert_eq!(
        (ev.operacao, ev.rowid, ev.versao),
        (Operacao::Exclusao, 3, 0)
    );
    let (_, externos) = Table::abrir_imagem(imagem).unwrap();
    assert_eq!(externos[0].1, b"anexo de x3@x.com");
}

/// O bidirecional: o evento aplicado guarda o carimbo e a origem de onde a
/// escrita NASCEU. Completar com o relogio da abertura elegeria o evento
/// errado no conflito, e origem zero o faria voltar para quem o mandou.
#[test]
fn o_evento_forcado_completa_com_o_carimbo_e_a_origem_do_nascimento() {
    let d = DirTemp::novo("diario-falha-forcado");
    let mut t = criar(&d.0);
    let antes = t.eventos().unwrap();
    t.forcar_proximo_evento(1_700_000_000_123, 4242);
    armar(&d.0, Onde::GravacaoDoDiario, 1);
    assert!(t.inserir(&linha(1, "f@x.com")).is_err());
    desarmar(&d.0);
    drop(t);
    let mut t = abrir(&d.0);
    let (ev, _) = t.diario_com_imagem(antes, 0).unwrap().pop().unwrap();
    assert_eq!((ev.carimbo, ev.origem), (1_700_000_000_123, 4242));
}

/// O comportamento velho: sem falha, nada de marca, e um evento por escrita.
#[test]
fn sem_falha_nada_muda() {
    let d = DirTemp::novo("diario-sem-falha");
    let mut t = criar(&d.0);
    for i in 1..=50 {
        t.inserir(&linha(i, &format!("s{i}@x.com"))).unwrap();
    }
    t.atualizar(7, &linha(7, "sete@x.com")).unwrap();
    t.excluir_de_vez(9, "teste").unwrap();
    t.sincronizar().unwrap();
    assert_eq!(t.eventos().unwrap(), 52);
    assert_eq!(marca_no_disco(&d.0), 0);
    drop(t);
    let mut t = abrir(&d.0);
    assert_eq!(t.eventos().unwrap(), 52, "a abertura inventou evento");
}

/// A biblioteca que pede para abrir SO PARA LER uma tabela devendo recebe
/// «precisa escrever»: completar e gravar, e sob a ficha compartilhada ha
/// outros leitores nos mesmos arquivos.
#[test]
fn abrir_para_ler_uma_tabela_devendo_pede_a_ficha_de_escrita() {
    let d = DirTemp::novo("diario-falha-ler");
    let mut t = criar(&d.0);
    armar(&d.0, Onde::GravacaoDoDiario, 1);
    assert!(t.inserir(&linha(1, "l@x.com")).is_err());
    desarmar(&d.0);
    drop(t);
    match Table::abrir_para_ler(&d.0, "contas").unwrap() {
        phxsql_store::table::SemEscrever::PrecisaEscrever(_) => {}
        phxsql_store::table::SemEscrever::Aberta(_) => {
            panic!("abriu para ler um diario que deve um evento")
        }
    }
    assert_eq!(marca_no_disco(&d.0), 1, "abrir para ler mexeu na marca");
}

/// Os IRMAOS: o observador que falha DEPOIS do `.reg` e ANTES do diario. A
/// exclusao de vez gravava o `.reason` com `?` antes do evento -- o `.reason`
/// que enchia pulava o diario, sem marca e sem queda, com o slot ja livre.
#[test]
fn a_exclusao_cujo_motivo_falha_ainda_anota_no_diario() {
    let d = DirTemp::novo("diario-falha-motivo");
    let mut t = criar(&d.0);
    for i in 1..=3 {
        t.inserir(&linha(i, &format!("m{i}@x.com"))).unwrap();
    }
    let antes = t.eventos().unwrap();
    armar(&d.0, Onde::GravacaoDoMotivo, 1);
    let e = t.excluir_de_vez(3, "teste").unwrap_err();
    desarmar(&d.0);
    assert!(e.to_string().contains("No space left"), "{e}");
    assert!(
        t.ler(3).unwrap().is_none(),
        "o slot nao saiu -- o teste nao mediu o caso"
    );
    let eventos = t.diario(antes, 0).unwrap();
    assert_eq!(
        eventos
            .iter()
            .map(|e| (e.operacao, e.rowid))
            .collect::<Vec<_>>(),
        vec![(Operacao::Exclusao, 3)],
        "a linha saiu do .reg sem evento no diario"
    );
}

/// O mesmo irmao na inclusao: o `.fts` que falha (a pagina despejada que o
/// disco recusa) voltava com `?` antes do evento.
#[test]
fn a_insercao_cujo_indice_de_texto_falha_ainda_anota_no_diario() {
    use phxsql_core::schema::IndiceDeTexto;
    let d = DirTemp::novo("diario-falha-fts");
    let e = Schema::new(
        "docs",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("corpo", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_indices_de_texto(vec![IndiceDeTexto::new("porCorpo", 1)])
    .unwrap();
    // Cache pequeno, ANTES de abrir (o teto vale para o que abre depois): o
    // `indexar` de um texto com muitos termos despeja pagina no meio, e e o
    // despejo que escreve.
    phxsql_store::ndx::definir_cache_paginas(4);
    let mut t = Table::criar(&d.0, e).unwrap();
    let corpo: String = (0..4000).map(|i| format!("termo{i:05} ")).collect();
    let antes = t.eventos().unwrap();
    let fts = d.0.join("docs.fts");
    armar(&fts, Onde::PaginaDoIndice, 1);
    let r = t.inserir(&[Value::Int(1), Value::Memo(corpo)]);
    desarmar(&fts);
    assert!(r.is_err(), "o .fts nao falhou -- o teste nao mediu o caso");
    assert_eq!(
        t.registros(),
        1,
        "a linha nao chegou ao .reg -- o teste nao mediu o caso"
    );
    let eventos = t.diario(antes, 0).unwrap();
    assert_eq!(
        eventos.iter().map(|e| e.operacao).collect::<Vec<_>>(),
        vec![Operacao::Inclusao],
        "a linha ficou no .reg sem evento no diario"
    );
}
