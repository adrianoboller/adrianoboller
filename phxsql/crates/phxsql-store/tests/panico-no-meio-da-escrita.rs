//! Pedidos 456 e 457: o disco depois de um panico NO MEIO de uma escrita.
//!
//! O `Drop` do `NdxFile` chamava `fechar()` sem perguntar nada, e num panico
//! no meio de uma escrita isso levava ao disco as paginas no estado em que o
//! panico as deixou e baixava o byte 52: a arvore rasgada ficava marcada
//! LIMPA, e o `recuperar` do arranque so reindexa tabela com o byte em 1. Um
//! `SIGKILL` no mesmo ponto deixava o byte em 1 -- o panico era pior que a
//! queda.
//!
//! # O que estes testes conferem, e em que ordem
//!
//! O DISCO, depois de reabrir, e nao o veredito da operacao que caiu. E a
//! ordem das perguntas e de proposito: primeiro a GARANTIA, perguntada a
//! tabela como ela voltou -- «a chave repetida entra?», «a mae sai com a filha
//! viva?», «a linha viva esta no indice?» --, e so depois a marca. Com o
//! defeito de pe, a tabela reaberta responde, e o vermelho tem de dizer QUAL
//! garantia ela quebrou; um vermelho que dissesse «o byte 52 esta em 0»
//! descreveria o mecanismo e esconderia o estrago.
//!
//! O panico e de verdade, por um gancho do motor que so os testes armam
//! (`ndx::panico_de_teste`), e a tabela morre no desenrolar, como no
//! servidor. O caminho do FFI -- o panico capturado e o `Drop` DEPOIS, com
//! `thread::panicking()` falso -- tem teste proprio aqui e outro pela ABI.
//!
//! O gancho so existe com `debug_assertions`: em `release` ele nao dispara, e
//! estes testes nao tem o que provar.
#![cfg(debug_assertions)]

mod comum;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use phxsql_core::error::PhxError;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::RowId;
use phxsql_store::ndx::panico_de_teste::{self, Ponto};
use phxsql_store::table::Table;

fn dir(nome: &str) -> comum::DirTemp {
    comum::DirTemp::novo(&format!("panico-{nome}"))
}

fn esquema_clientes() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn cliente(id: i64, nome: &str) -> Vec<Value> {
    vec![Value::Int(id), Value::Str(nome.into())]
}

/// `clientes` com os ids `1..=n`, sincronizada e FECHADA: o byte 52 no disco
/// esta em 0, e a sessao seguinte comeca limpa.
fn semear(d: &Path, n: i64) -> Vec<RowId> {
    let mut t = Table::criar(d, esquema_clientes()).unwrap();
    let rowids = (1..=n)
        .map(|i| t.inserir(&cliente(i, &format!("C{i}"))).unwrap())
        .collect();
    t.sincronizar().unwrap();
    rowids
}

/// O byte 52 do `.ndx`, lido do ARQUIVO -- nao do que a tabela acha dele.
fn byte_52(d: &Path, tabela: &str) -> u8 {
    let b = std::fs::read(d.join(format!("{tabela}.ndx"))).unwrap();
    b[52]
}

/// Roda `f` esperando o panico do gancho. Se ele nao vier, o teste nao
/// provou nada, e diz isso em vez de seguir.
fn esperar_panico(ponto: Ponto, f: impl FnOnce()) {
    panico_de_teste::armar(ponto);
    let r = catch_unwind(AssertUnwindSafe(f));
    panico_de_teste::desarmar();
    assert!(
        r.is_err(),
        "o gancho {ponto:?} nao disparou: o teste nao chegou ao meio da escrita"
    );
}

/// Toda linha viva no `.reg` tem de ser achada pela chave, no indice.
///
/// E a pergunta que a tabela reaberta tem de responder CERTO se respondeu
/// alguma coisa: a busca pela chave de cada linha que o `.reg` diz viva.
fn conferir_linhas_no_indice(t: &mut Table, indice: &str) {
    for (rowid, linha) in t.varrer().unwrap() {
        match t.buscar(indice, &[linha[0].clone()]) {
            Ok(achados) if achados.contains(&rowid) => {}
            Ok(achados) => panic!(
                "linha viva fora do indice: a linha {rowid} ({:?}) esta viva no \
                 .reg e o indice {indice} devolve {achados:?} pela chave dela",
                linha[0]
            ),
            Err(e) => panic!(
                "linha viva fora do indice: a linha {rowid} ({:?}) esta viva no \
                 .reg e o indice {indice} nao a acha -- responde: {e}",
                linha[0]
            ),
        }
    }
}

/// Depois do `reindexar`: a contagem pelo `.reg` e a contagem por cada indice
/// batem, sem rowid repetido, e o id continua unico entre as vivas.
fn conferir_reindexada(t: &mut Table, indices: &[&str]) {
    t.reindexar()
        .expect("o reindexar tem de consertar a tabela marcada");
    let vivas = t.varrer().unwrap();
    let mut pelo_reg: Vec<RowId> = vivas.iter().map(|(r, _)| *r).collect();
    pelo_reg.sort_unstable();
    let mut ids: Vec<String> = vivas.iter().map(|(_, l)| format!("{:?}", l[0])).collect();
    ids.sort();
    let antes = ids.len();
    ids.dedup();
    assert_eq!(
        antes,
        ids.len(),
        "chave duplicada entre as linhas vivas do .reg: {ids:?}"
    );
    for indice in indices {
        let mut pelo_indice = t.varrer_indice(indice).unwrap();
        pelo_indice.sort_unstable();
        assert_eq!(
            pelo_indice,
            pelo_reg,
            "contagem pelo .reg ({}) diferente da contagem pelo indice {indice} ({})",
            pelo_reg.len(),
            pelo_indice.len()
        );
    }
}

/// A marca, e a recusa que ela manda. So se pergunta DEPOIS das garantias.
fn conferir_marcada(d: &Path, tabela: &str) -> Table {
    assert_eq!(
        byte_52(d, tabela),
        1,
        "{tabela}.ndx: a escrita interrompida tinha de deixar o byte 52 em 1"
    );
    let t = Table::abrir(d, tabela).unwrap();
    assert!(
        t.indice_precisa_reconstruir(),
        "{tabela}: aberta com o byte 52 em 1, a tabela tinha de pedir reconstrucao"
    );
    t
}

// ====================================================== os quatro pontos

/// O panico no meio da DIVISAO de uma folha: a metade esquerda ja no cache, a
/// direita e o pai ainda nao. O `Drop` de antes gravava a folha com metade
/// das chaves e a vizinha nova em branco, e as linhas da metade que sumiu
/// ficavam vivas no `.reg` sem o indice acha-las.
#[test]
fn panico_no_meio_da_divisao_nao_grava_a_arvore_rasgada() {
    let d = dir("divisao");
    semear(&d, 10);

    esperar_panico(Ponto::NoMeioDaDivisao, || {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        // Uma folha de 4 KiB cabe ~240 destas chaves: a divisao vem antes
        // do fim do laco, e e nela que o gancho dispara.
        for i in 11..=5_000 {
            t.inserir(&cliente(i, "x")).unwrap();
        }
    });

    let mut t = Table::abrir(&d, "clientes").unwrap();
    if !t.indice_precisa_reconstruir() {
        conferir_linhas_no_indice(&mut t, "porId");
    }
    drop(t);
    let mut t = conferir_marcada(&d, "clientes");
    conferir_reindexada(&mut t, &["porId"]);
    conferir_linhas_no_indice(&mut t, "porId");
}

/// O panico depois do contador do `.reg` e antes da primeira chave: a linha
/// ja esta viva para quem le o `.reg`, e o indice unico nao a conhece.
///
/// E o ponto que prova as DUAS camadas. Nenhuma pagina do `.ndx` sujou nesta
/// sessao, entao so a subida do byte 52 ANTES do `.reg` (camada 1) deixa o
/// disco dizendo a verdade -- e so o `Drop` que nao a baixa (camada 0)
/// impede o desenrolar de apagar essa verdade.
#[test]
fn panico_depois_do_contador_nao_aceita_chave_duplicada() {
    let d = dir("contador");
    semear(&d, 5);

    esperar_panico(Ponto::InserirDepoisDoContador, || {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        t.inserir(&cliente(6, "C6")).unwrap();
    });

    let mut t = Table::abrir(&d, "clientes").unwrap();
    if let Ok(r) = t.inserir(&cliente(6, "de novo")) {
        panic!(
            "chave duplicada aceita no indice unico: o id 6 ja esta vivo no .reg \
             (a linha do panico) e entrou de novo como a linha {r}"
        );
    }
    drop(t);
    let mut t = conferir_marcada(&d, "clientes");
    conferir_reindexada(&mut t, &["porId"]);
    assert_eq!(t.buscar("porId", &[Value::Int(6)]).unwrap().len(), 1);
    assert!(
        matches!(
            t.inserir(&cliente(6, "de novo")),
            Err(PhxError::Duplicado(_))
        ),
        "depois do reindexar, o id 6 repetido tinha de ser recusado"
    );
}

/// O `atualizar` que troca a chave, com o panico entre o slot regravado e a
/// troca no indice: o `.reg` diz 30, o indice ainda diz 3.
#[test]
fn panico_depois_do_reg_no_atualizar_nao_aceita_chave_duplicada() {
    let d = dir("atualizar");
    let rowids = semear(&d, 5);
    let alvo = rowids[2];

    esperar_panico(Ponto::AtualizarDepoisDoReg, || {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        t.atualizar(alvo, &cliente(30, "C3")).unwrap();
    });

    let mut t = Table::abrir(&d, "clientes").unwrap();
    if let Ok(r) = t.inserir(&cliente(30, "outro")) {
        panic!(
            "chave duplicada aceita no indice unico: a linha {alvo} ja tem o id 30 \
             no .reg (o atualizar do panico) e o id 30 entrou de novo como a linha {r}"
        );
    }
    drop(t);
    let mut t = conferir_marcada(&d, "clientes");
    conferir_reindexada(&mut t, &["porId"]);
    assert_eq!(t.buscar("porId", &[Value::Int(30)]).unwrap(), vec![alvo]);
    assert!(t.buscar("porId", &[Value::Int(3)]).unwrap().is_empty());
}

fn esquema_pedidos() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            // A busca reversa da mae pergunta a ESTE indice.
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )])
    .unwrap()
}

/// O `excluir_de_vez` da FILHA, com o panico entre as chaves tiradas e o slot
/// liberado: a filha continua viva no `.reg` e sumiu do indice que a mae
/// consulta antes de morrer. Com o `Drop` de antes, a regra primordial caia:
/// a mae saia de vez com a filha viva apontando para ela.
#[test]
fn panico_entre_remover_e_excluir_nao_deixa_apagar_a_mae() {
    let d = dir("excluir");
    let mae = {
        let e = Schema::new(
            "clientes",
            vec![Column::new("id", ColumnType::Int4).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut m = Table::criar(&d, e).unwrap();
        let r = m.inserir(&[Value::Int(1)]).unwrap();
        m.sincronizar().unwrap();
        r
    };
    let filha = {
        let mut f = Table::criar(&d, esquema_pedidos()).unwrap();
        let r = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
        f.sincronizar().unwrap();
        r
    };

    esperar_panico(Ponto::ExcluirEntreRemoverEExcluir, || {
        let mut f = Table::abrir(&d, "pedidos").unwrap();
        f.excluir_de_vez(filha, "teste").unwrap();
    });

    let mut m = Table::abrir(&d, "clientes").unwrap();
    if let Ok(true) = m.excluir_de_vez(mae, "teste") {
        let mut f = Table::abrir(&d, "pedidos").unwrap();
        let viva = f
            .varrer()
            .unwrap()
            .into_iter()
            .any(|(_, l)| l[1] == Value::Int(1));
        assert!(
            !viva,
            "mae apagada com filha viva: o cliente 1 saiu de vez e o pedido 10 \
             continua vivo no .reg apontando para ele"
        );
    }
    drop(m);

    let mut f = conferir_marcada(&d, "pedidos");
    conferir_reindexada(&mut f, &["porId", "porCliente"]);
    drop(f);
    // Com o indice da filha em dia, a mae recusa pela regra, e continua viva.
    let mut m = Table::abrir(&d, "clientes").unwrap();
    assert!(
        m.excluir_de_vez(mae, "teste").is_err(),
        "com a filha reindexada e viva, a mae tinha de recusar sair"
    );
    assert!(m.ler(mae).unwrap().is_some());
}

/// O irmao que chama as mesmas pecas: o `reindexar` recria o `.ndx` VAZIO e
/// so depois monta as arvores. Um panico no meio deixava o `Drop` gravar o
/// arquivo vazio marcado limpo -- a tabela inteira fora do indice, calada, e
/// justamente pelo caminho que existe para consertar indice.
#[test]
fn panico_no_meio_do_reindexar_nao_grava_o_indice_vazio() {
    let d = dir("reindexar");
    semear(&d, 5);

    esperar_panico(Ponto::NoMeioDoReindexar, || {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        t.reindexar().unwrap();
    });

    let mut t = Table::abrir(&d, "clientes").unwrap();
    if !t.indice_precisa_reconstruir() {
        conferir_linhas_no_indice(&mut t, "porId");
    }
    drop(t);
    let mut t = conferir_marcada(&d, "clientes");
    conferir_reindexada(&mut t, &["porId"]);
    assert_eq!(t.varrer_indice("porId").unwrap().len(), 5);
}

// ================================================= o caminho do FFI, aqui

/// O panico capturado, e o `Drop` DEPOIS, fora do desenrolar -- e o que o
/// `punho::com` do FFI faz, com `thread::panicking()` falso quando o
/// `liberar` roda. Um conserto que decidisse por `panicking()` passaria nos
/// quatro testes de cima e cairia neste.
#[test]
fn panico_capturado_e_drop_depois_tambem_nao_grava_o_meio() {
    let d = dir("capturado");
    semear(&d, 5);

    let mut t = Table::abrir(&d, "clientes").unwrap();
    panico_de_teste::armar(Ponto::InserirDepoisDoContador);
    let r = catch_unwind(AssertUnwindSafe(|| t.inserir(&cliente(6, "C6"))));
    panico_de_teste::desarmar();
    assert!(r.is_err(), "o gancho nao disparou");
    assert!(!std::thread::panicking());
    drop(t);

    let mut t = Table::abrir(&d, "clientes").unwrap();
    if let Ok(r) = t.inserir(&cliente(6, "de novo")) {
        panic!(
            "chave duplicada aceita no indice unico: o id 6 ja esta vivo no .reg \
             (a linha do panico capturado) e entrou de novo como a linha {r}"
        );
    }
    drop(t);
    let mut t = conferir_marcada(&d, "clientes");
    conferir_reindexada(&mut t, &["porId"]);
}

// ============================================================ pedido 457

/// O `sincronizar` de um `.ndx` que ABRIU sujo nao pode sair limpo.
///
/// A marca aqui e a de verdade: deixada por um descritor vivo com escrita
/// pendente (o CRC do cabecalho recusa marca virada a mao). Um segundo
/// descritor abre sujo, faz um `atualizar` que nao troca chave -- nada nele
/// toca o indice, entao nada recusa -- e fecha a janela com `sincronizar`.
/// Depois o primeiro cai sem descarregar (`forget`, a queda do escritor).
#[test]
fn sincronizar_nao_limpa_o_indice_que_abriu_sujo() {
    let d = dir("457");
    let rowids = semear(&d, 5);

    let mut escritor = Table::abrir(&d, "clientes").unwrap();
    escritor.inserir(&cliente(6, "C6")).unwrap();
    assert_eq!(
        byte_52(&d, "clientes"),
        1,
        "a escrita pendente sobe a marca"
    );
    {
        let mut outro = Table::abrir(&d, "clientes").unwrap();
        assert!(outro.indice_precisa_reconstruir());
        outro
            .atualizar(rowids[0], &cliente(1, "nome novo"))
            .unwrap();
        outro.sincronizar().unwrap();
    }
    std::mem::forget(escritor);

    let mut t = Table::abrir(&d, "clientes").unwrap();
    if let Ok(r) = t.inserir(&cliente(6, "de novo")) {
        panic!(
            "chave duplicada aceita no indice unico: o id 6 esta vivo no .reg \
             (a escrita do descritor que caiu) e entrou de novo como a linha {r} \
             -- o sincronizar do outro descritor limpou a marca sem reconstruir"
        );
    }
    drop(t);
    let mut t = conferir_marcada(&d, "clientes");
    conferir_reindexada(&mut t, &["porId"]);
}

// ================================================ o comportamento de antes

/// A escrita que termina continua descendo a marca ao fechar: a camada 0 nao
/// pode virar «toda tabela escrita abre marcada».
#[test]
fn sem_panico_a_marca_desce_ao_fechar_como_antes() {
    let d = dir("sem-panico");
    let rowids = semear(&d, 5);
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        t.inserir(&cliente(6, "C6")).unwrap();
        t.atualizar(rowids[1], &cliente(20, "C2")).unwrap();
        t.excluir_de_vez(rowids[3], "teste").unwrap();
    }
    assert_eq!(byte_52(&d, "clientes"), 0);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert!(!t.indice_precisa_reconstruir());
    conferir_linhas_no_indice(&mut t, "porId");
}

/// «Tabela cheia» recusa ANTES de gravar: o indice continua em dia, a tabela
/// continua respondendo, e o fechamento desce a marca. Fechar a janela como
/// interrompida em todo erro do `.reg` faria uma recusa comum exigir
/// `reparar indice`.
#[test]
fn tabela_cheia_nao_deixa_o_indice_marcado() {
    let d = dir("cheia");
    {
        let e = esquema_clientes()
            .com_paginacao(Paginacao::nova(3, 2).unwrap())
            .unwrap();
        let mut t = Table::criar(&d, e).unwrap();
        for i in 1..=6 {
            t.inserir(&cliente(i, "C")).unwrap();
        }
        let e = t.inserir(&cliente(7, "C7")).unwrap_err();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "erro foi {e}");
        t.verificar()
            .expect("a recusa nao pode deixar o indice recusando");
    }
    assert_eq!(byte_52(&d, "clientes"), 0);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert!(!t.indice_precisa_reconstruir());
    conferir_linhas_no_indice(&mut t, "porId");
}
