//! `ao_alterar`: a metade da SP000008 que estava DECLARADA e nao acontecia.
//!
//! O defeito, medido antes de mexer: a mae com `ao_alterar: Cascata` mudava a
//! coluna referenciada, a filha continuava apontando para o valor velho, e
//! `buscar` na mae por aquele valor devolvia **zero linhas** -- orfa, e calada,
//! que e o jeito pior. O campo aparecia em `schema.rs` (guardar/serializar),
//! no `cli` (mostrar) e em ponto nenhum de escrita.
//!
//! Estes testes travam os TRES sentidos, porque so o primeiro nao prova nada:
//! a cascata acontece, a alteracao que nao mexe na chave nao paga nada, e quem
//! nunca pediu conferencia continua exatamente como antes.

mod comum;
use phxsql_core::error::PhxError;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn dir(nome: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("casc-{nome}"))
}

/// A mae: `id` indexado (a cascata precisa dele dos dois lados) e um `nome`
/// FORA de qualquer indice -- e ele que prova o portao de custo zero.
fn mae(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A filha, com o indice da coluna da chave -- sem ele o motor RECUSA.
fn filha_com(
    d: &std::path::Path,
    nome: &str,
    acao: AcaoRi,
    conferindo: bool,
    com_indice: bool,
) -> Table {
    let mut indices = vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()];
    if com_indice {
        indices.push(IndexDef::new("porCliente", vec![IndexColumn::asc(1)]));
    }
    let e = Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        indices,
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .ao_alterar(acao)
    .conferindo(conferindo)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn filha(d: &std::path::Path, acao: AcaoRi) -> Table {
    filha_com(d, "pedidos", acao, true, true)
}

/// O valor da coluna `cliente_id` da linha `r` de `f`, relido do disco.
fn aponta_para(f: &mut Table, r: u64) -> Value {
    f.ler(r).unwrap().unwrap()[1].clone()
}

// ---------------------------------------------------------------------------
// O coracao: a filha acompanha
// ---------------------------------------------------------------------------

/// Prova real: trocar `AcaoRi::Cascata` por `NaoFazerNada` no filtro de
/// `planejar_ao_alterar` -- ou apagar a chamada a `aplicar_ao_alterar` do fim
/// do `atualizar` -- devolve `Int(1)` aqui, que e exatamente o defeito medido.
#[test]
fn a_filha_acompanha_a_chave_que_a_mae_mudou() {
    let d = dir("acompanha");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, AcaoRi::Cascata);
    let p1 = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    let p2 = f.inserir(&[Value::Int(11), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect("a mae nao conseguiu alterar a chave");

    // Reabre a filha: o que importa e o que ficou NO DISCO, e nao o que o
    // descritor do teste tem em memoria.
    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(
        aponta_para(&mut f, p1),
        Value::Int(7),
        "a filha 1 ficou orfa"
    );
    assert_eq!(
        aponta_para(&mut f, p2),
        Value::Int(7),
        "a filha 2 ficou orfa"
    );
    // E o INDICE da filha acompanhou: sem isto a linha estaria certa e
    // inalcancavel, que e o defeito que so aparece na busca.
    assert_eq!(
        f.buscar("porCliente", &[Value::Int(7)]).unwrap().len(),
        2,
        "o indice da filha ficou apontando para a chave velha"
    );
    assert!(
        f.buscar("porCliente", &[Value::Int(1)]).unwrap().is_empty(),
        "a chave velha continua no indice da filha"
    );
}

/// A filha de OUTRA mae nao se mexe.
///
/// Sem este teste, uma cascata que trocasse a coluna de todas as linhas da
/// filha passaria no de cima.
#[test]
fn a_filha_de_outra_linha_nao_se_mexe() {
    let d = dir("outra-linha");
    let mut m = mae(&d);
    let um = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.inserir(&[Value::Int(2), Value::Str("Bia".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, AcaoRi::Cascata);
    let da_um = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    let da_dois = f.inserir(&[Value::Int(11), Value::Int(2)]).unwrap();
    let sem_mae = f.inserir(&[Value::Int(12), Value::Null]).unwrap();
    f.sincronizar().unwrap();

    m.atualizar(um, &[Value::Int(7), Value::Str("Ana".into())])
        .unwrap();

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(aponta_para(&mut f, da_um), Value::Int(7));
    assert_eq!(
        aponta_para(&mut f, da_dois),
        Value::Int(2),
        "a filha da mae 2 foi arrastada junto"
    );
    assert_eq!(
        aponta_para(&mut f, sem_mae),
        Value::Null,
        "a filha sem mae foi arrastada junto"
    );
}

/// A cascata segue a CADEIA: mae -> filha -> neta.
///
/// Sai de graca por a filha ser gravada com um `atualizar` inteiro, e nao por
/// um atalho -- e este teste e o que trava essa decisao.
#[test]
fn a_cascata_alcanca_a_neta() {
    let d = dir("neta");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    // A filha referencia `clientes.id` pela coluna 1, e e referenciada pela
    // neta na coluna 0 (o `id` dela) -- a cadeia so anda porque a coluna que a
    // neta aponta e a `id` da filha, que nao muda. Entao a neta aponta para a
    // COLUNA DA CHAVE da filha: quem muda e ela.
    let mut f = filha_com(&d, "pedidos", AcaoRi::Cascata, true, true);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    // A neta aponta para `pedidos.cliente_id`, que e o que a cascata muda.
    let e = Schema::new(
        "itens",
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
        "fk_pedido",
        vec![1],
        "pedidos",
        vec!["cliente_id".into()],
    )
    .ao_alterar(AcaoRi::Cascata)])
    .unwrap();
    let mut n = Table::criar(&d, e).unwrap();
    let i = n.inserir(&[Value::Int(100), Value::Int(1)]).unwrap();
    n.sincronizar().unwrap();

    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect("a mae nao alterou");

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    let mut n = Table::abrir(&d, "itens").unwrap();
    assert_eq!(
        aponta_para(&mut f, p),
        Value::Int(7),
        "a filha nao acompanhou"
    );
    assert_eq!(
        aponta_para(&mut n, i),
        Value::Int(7),
        "a cadeia parou na filha: a neta ficou orfa"
    );
}

// ---------------------------------------------------------------------------
// As outras acoes: restringir e anular
// ---------------------------------------------------------------------------

/// `ao_alterar: restringir` RECUSA -- e recusa ANTES de gravar a mae.
///
/// A segunda afirmacao e a que importa: um teste que so conferisse o veredito
/// passaria com a recusa acontecendo depois do dano. Ver a licao do revisor de
/// prova real -- «o numero certo saiu quando ela passou a medir QUANTO foi
/// lido, e nao SE recusou».
#[test]
fn restringir_recusa_e_a_mae_nao_se_mexe() {
    let d = dir("restringir");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, AcaoRi::Restringir);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    let e = m
        .atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect_err("restringir deixou a mae mudar a chave");
    let txt = e.to_string();
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "familia errada: {txt}"
    );
    assert!(txt.contains("pedidos"), "nao diz ONDE esta a filha: {txt}");
    assert!(txt.contains("restringir"), "nao diz por que recusou: {txt}");

    // O dano: a mae tem de estar INTACTA no disco, e a filha tambem.
    m.sincronizar().unwrap();
    let mut m2 = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        m2.ler(r).unwrap().unwrap()[0],
        Value::Int(1),
        "a mae gravou antes de a recusa acontecer"
    );
    assert_eq!(
        m2.buscar("porId", &[Value::Int(7)]).unwrap().len(),
        0,
        "a chave nova entrou no indice da mae apesar da recusa"
    );
    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(aponta_para(&mut f, p), Value::Int(1));
}

/// `restringir` sem filha nenhuma nao recusa nada.
///
/// A outra metade: sem ela, um portao que recusasse TODA alteracao de chave
/// passaria no teste de cima e tornaria a tabela inutil.
#[test]
fn restringir_sem_filha_deixa_alterar() {
    let d = dir("restringir-sem-filha");
    let mut m = mae(&d);
    let com = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    let sem = m
        .inserir(&[Value::Int(2), Value::Str("Bia".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, AcaoRi::Restringir);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    m.atualizar(sem, &[Value::Int(8), Value::Str("Bia".into())])
        .expect("a linha sem filha foi barrada");
    // E a que tem filha continua barrada, na MESMA tabela -- senao a de cima
    // poderia estar passando por a tabela inteira estar destrancada.
    assert!(m
        .atualizar(com, &[Value::Int(7), Value::Str("Ana".into())])
        .is_err());
}

/// `ao_alterar: anular` deixa a filha sem mae, em vez de arrasta-la.
#[test]
fn anular_deixa_a_filha_sem_mae() {
    let d = dir("anular");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, AcaoRi::AnularCampos);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect("a mae nao alterou");

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(
        aponta_para(&mut f, p),
        Value::Null,
        "anular copiou a chave nova em vez de anular"
    );
}

// ---------------------------------------------------------------------------
// O que NAO muda -- e e aqui que a guarda nova se prova inofensiva
// ---------------------------------------------------------------------------

/// O teste do comportamento VELHO, o que mais importa numa guarda nova: quem
/// nunca pediu conferencia continua alterando a chave como sempre alterou, e a
/// filha continua onde estava.
#[test]
fn sem_conferir_nada_muda() {
    let d = dir("sem-conferir");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    // `verificar: false` E sem o indice da chave na filha: se a cascata olhasse
    // esta chave, recusaria por falta de indice em vez de ignora-la.
    let mut f = filha_com(&d, "pedidos", AcaoRi::Cascata, false, false);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect("a guarda nova quebrou quem nunca pediu nada");

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(
        aponta_para(&mut f, p),
        Value::Int(1),
        "a cascata mexeu numa chave que nao pediu conferencia"
    );
}

/// Alterar o que a chave NAO ve nao recusa nem cascateia.
///
/// A filha e de proposito a que NAO tem indice na coluna da chave: se a
/// alteracao do `nome` chegasse ate a conferencia do indice, ela recusaria
/// dizendo qual falta -- e recusaria uma alteracao que nao tem nada com a
/// chave.
///
/// Este teste NAO prova o portao de custo zero, e a distincao custou uma prova
/// falsa: repondo o defeito -- tirando `alguma_coluna_indexada_mudou` --, ele
/// continuava passando, porque a chave `id` nao tinha mudado e a varredura
/// desistia antes do indice. Portao e desempenho, e desempenho se prova
/// medindo: ver `tests/portao-do-ao-alterar.rs`.
#[test]
fn alterar_o_que_a_chave_nao_ve_nao_recusa_nem_cascateia() {
    let d = dir("fora-da-chave");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha_com(&d, "pedidos", AcaoRi::Cascata, true, false);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    m.atualizar(r, &[Value::Int(1), Value::Str("Ana Maria".into())])
        .expect("alterar coluna fora da chave foi recusado pelo braco do ao_alterar");

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(
        aponta_para(&mut f, p),
        Value::Int(1),
        "a filha se mexeu numa alteracao que nao tocou na chave"
    );
}

/// Sem indice na filha, a alteracao da chave e RECUSADA dizendo qual falta --
/// e nao varrida escondido.
///
/// E a mesma exigencia dos dois lados que o `excluir` ja faz, e pelo mesmo
/// motivo: uma varredura da tabela de filhas inteira escondida dentro de um
/// `atualizar` que parece barato e uma lentidao que ninguem explica.
#[test]
fn sem_indice_na_filha_recusa_dizendo_qual_falta() {
    let d = dir("sem-indice");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha_com(&d, "pedidos", AcaoRi::Cascata, true, false);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    let e = m
        .atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect_err("a alteracao passou sem indice na filha");
    let txt = e.to_string();
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "familia errada: {txt}"
    );
    assert!(
        txt.contains("cliente_id"),
        "nao diz por qual coluna falta indice: {txt}"
    );
    assert!(txt.contains("crie o indice"), "nao diz o que fazer: {txt}");
}

/// Alterar coluna INDEXADA que a chave nao referencia nao regrava a filha.
///
/// Aqui o portao ABRE -- a coluna esta num indice -- e a varredura das irmas
/// acontece de verdade. O que segura a filha e o outro teste, o de a chave nao
/// ter mudado; sem ele a cascata regravaria toda filha com o MESMO valor.
///
/// E isso nao passaria despercebido, so despercebido pelo VALOR: a versao do
/// registro sobe, o diario ganha um evento e a trilha ganha uma alteracao que
/// nao aconteceu. Por isso este teste mede a VERSAO, e nao o conteudo -- medir
/// o conteudo daria igual com o defeito reposto, que e a prova falsa que esta
/// bateria ja pagou uma vez.
#[test]
fn indexada_mas_nao_referenciada_nao_regrava_a_filha() {
    let d = dir("indexada-sem-chave");
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("apelido", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porApelido", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    let mut m = Table::criar(&d, e).unwrap();
    let r = m.inserir(&[Value::Int(1), Value::Str("a".into())]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, AcaoRi::Cascata);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    let antes = f.versao(p).unwrap();

    // `apelido` esta num indice e nao e referenciado por chave nenhuma.
    m.atualizar(r, &[Value::Int(1), Value::Str("b".into())])
        .unwrap();

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(
        f.versao(p).unwrap(),
        antes,
        "a filha foi regravada por uma alteracao que nao mexeu na chave"
    );
}

/// A chave nasce com `ao_alterar: Cascata` pelas DUAS portas de entrada.
///
/// Era a divergencia medida nesta frente: o JSON do servidor entregava
/// `Cascata` quando o campo vinha ausente e o `ForeignKey::new` entregava
/// `Restringir` -- a mesma tabela nascia com integridade diferente conforme
/// quem a criasse. Com a cascata executando, isso deixou de ser detalhe.
#[test]
fn a_chave_nasce_com_cascata_ao_alterar() {
    let fk = ForeignKey::new("f", vec![1], "clientes", vec!["id".into()]);
    assert_eq!(fk.ao_alterar, AcaoRi::Cascata);
    assert_eq!(fk.ao_excluir, AcaoRi::Restringir);
}

// ---------------------------------------------------------------------------
// O caminho da RECUPERACAO: `recascatear`
// ---------------------------------------------------------------------------

/// `recascatear` conferia a arvore TARDE -- e gravava a primeira filha antes.
///
/// # O defeito, e por que ele so aparece no encontro de dois pedidos
///
/// O 168 pos `recascatear` na recuperacao; o 169 pos `conferir_a_arvore` no
/// `atualizar`, para recusar a arvore inteira ANTES de a mae ir ao disco.
/// `recascatear` nasceu primeiro e ficou sem a conferencia: ele planejava o
/// nivel 1 e ja aplicava.
///
/// A diferenca so e visivel com **duas filhas no nivel 1**, porque cada
/// `filha.atualizar` confere a propria sub-arvore: com uma filha so, a recusa
/// da neta chega antes de qualquer escrita e nada denuncia o buraco. Com duas,
/// a primeira e gravada e so entao a segunda recusa -- cascata pela metade, e
/// na recuperacao, que e onde ninguem esta olhando.
///
/// # A ordem NAO e sorteio, e e isso que torna o teste honesto
///
/// `catalogo::tabelas_em` ordena os nomes, entao `aaa_pedidos` e sempre
/// planejada antes de `zzz_pedidos`. Sem isso o teste passaria em metade das
/// corridas -- e teste que passa por engano e pior que teste que falta.
///
/// # Prova real
///
/// Tirar o `conferir_a_arvore` de `recascatear` faz `aaa_pedidos` voltar
/// `Int(7)` aqui: gravada, com a cascata recusada logo depois.
#[test]
fn a_recascata_recusa_antes_de_gravar_a_primeira_filha() {
    let d = dir("recascatear-arvore");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    // A chave de destino ja existe: a queda foi DEPOIS de a mae ir ao disco, e
    // e esse o estado que `recascatear` encontra. Sem ela, a filha recusaria
    // pela propria chave estrangeira e o teste passaria pelo motivo errado.
    m.inserir(&[Value::Int(7), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();

    let mut boa = filha_com(&d, "aaa_pedidos", AcaoRi::Cascata, true, true);
    let pb = boa.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    boa.sincronizar().unwrap();

    let mut ruim = filha_com(&d, "zzz_pedidos", AcaoRi::Cascata, true, true);
    ruim.inserir(&[Value::Int(20), Value::Int(1)]).unwrap();
    ruim.sincronizar().unwrap();

    // A neta pendura RESTRINGIR na SEGUNDA filha: quem recusa esta no nivel 2,
    // e so se descobre descendo.
    let e = Schema::new(
        "itens",
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
        "fk_pedido",
        vec![1],
        "zzz_pedidos",
        vec!["cliente_id".into()],
    )
    .ao_alterar(AcaoRi::Restringir)])
    .unwrap();
    let mut n = Table::criar(&d, e).unwrap();
    n.inserir(&[Value::Int(100), Value::Int(1)]).unwrap();
    n.sincronizar().unwrap();

    m.recascatear(
        &[Value::Int(1), Value::Str("Ana".into())],
        &[Value::Int(7), Value::Str("Ana".into())],
    )
    .expect_err("a neta restringe: a recascata tinha de recusar");

    let mut boa = Table::abrir(&d, "aaa_pedidos").unwrap();
    assert_eq!(
        aponta_para(&mut boa, pb),
        Value::Int(1),
        "a recusa chegou DEPOIS de gravar a primeira filha: cascata pela metade"
    );
}

// ---------------------------------------------------------------------------
// Auto-referencia: RECUSA, e nao silencio
// ---------------------------------------------------------------------------

/// `funcionarios.chefe_id -> funcionarios.id`: uma tabela que aponta para si.
fn hierarquia(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "funcionarios",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("chefe_id", ColumnType::Int4),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porChefe", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_chefe",
        vec![1],
        "funcionarios",
        vec!["id".into()],
    )
    .ao_alterar(AcaoRi::Cascata)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A auto-referencia nao cascateia -- e ate 03/09/2026 ela saia do plano em
/// SILENCIO, deixando a subordinada apontando para a chave velha com o
/// `atualizar` devolvendo `Ok`.
///
/// # Por que recusar, e nao passar
///
/// Os dois motores de referencia escolheram o oposto do silencio, com a mesma
/// frase: «it acts like RESTRICT». E a mesma lei desta casa -- orfa que
/// ninguem ve e pior que orfa que da erro. Ver `docs/INTEGRIDADE.md` §7.4.
///
/// # Prova real
///
/// Trocar a recusa por `continue` -- que e como o codigo era -- faz o
/// `atualizar` devolver `Ok` aqui, e a subordinada volta `Int(1)`: a orfa
/// calada, que e o defeito de origem.
#[test]
fn a_auto_referencia_recusa_em_vez_de_orfanar_calada() {
    let d = dir("auto-recusa");
    let mut t = hierarquia(&d);
    let chefe = t
        .inserir(&[Value::Int(1), Value::Null, Value::Str("Ana".into())])
        .unwrap();
    t.sincronizar().unwrap();
    let sub = t
        .inserir(&[Value::Int(2), Value::Int(1), Value::Str("Bia".into())])
        .unwrap();
    t.sincronizar().unwrap();

    let erro = t
        .atualizar(
            chefe,
            &[Value::Int(7), Value::Null, Value::Str("Ana".into())],
        )
        .expect_err("a auto-referencia tinha de recusar, e nao passar calada");
    let texto = erro.to_string();
    assert!(
        texto.contains("fk_chefe") && texto.contains("funcionarios"),
        "a recusa tem de NOMEAR a chave e a tabela; veio: {texto}"
    );

    // E o principal: a recusa acontece ANTES de gravar. A subordinada continua
    // apontando para um chefe que existe, e nao para um que sumiu.
    let mut t = Table::abrir(&d, "funcionarios").unwrap();
    assert_eq!(
        t.ler(sub).unwrap().unwrap()[1],
        Value::Int(1),
        "a subordinada mudou apesar da recusa"
    );
    assert_eq!(
        t.ler(chefe).unwrap().unwrap()[0],
        Value::Int(1),
        "a mae foi gravada apesar da recusa"
    );
}

/// O teste que mais importa numa guarda nova: o do comportamento VELHO.
///
/// Quem tem auto-referencia declarada e altera uma coluna que NAO e a
/// referenciada continua exatamente como antes. A `chefe_id` e indexada e e a
/// coluna LOCAL da chave, entao ela passa pelo portao de
/// `alguma_coluna_indexada_mudou` e chega no codigo novo -- e e por isso que
/// ela serve de controle, e nao o `nome`, que pararia antes de chegar la.
#[test]
fn mudar_coluna_que_nao_e_a_referenciada_continua_passando() {
    let d = dir("auto-controle");
    let mut t = hierarquia(&d);
    t.inserir(&[Value::Int(1), Value::Null, Value::Str("Ana".into())])
        .unwrap();
    t.sincronizar().unwrap();
    let sub = t
        .inserir(&[Value::Int(2), Value::Int(1), Value::Str("Bia".into())])
        .unwrap();
    t.sincronizar().unwrap();

    t.atualizar(sub, &[Value::Int(2), Value::Null, Value::Str("Bia".into())])
        .expect("trocar o chefe nao mexe na chave referenciada: tinha de passar");

    let mut t = Table::abrir(&d, "funcionarios").unwrap();
    assert_eq!(t.ler(sub).unwrap().unwrap()[1], Value::Null);
}

/// A procura pelas filhas nao pode mandar reparar um `.ndx` SAO.
///
/// # O irmao do pedido 176, e o padrao que ja apareceu tres vezes num dia
///
/// O 176 consertou o recado do `conferir_fks` -- o lado que pergunta «existe
/// esta mae?». Este e o lado oposto: `planejar_ao_alterar` pergunta «quem
/// aponta para esta mae?», e recusava com o MESMO texto cru embrulhado, sob um
/// comentario que tambem afirmava que o erro cru era ruim. Envolver nao e
/// substituir, e o comentario que se declara resolvido e o motivo de ninguem
/// olhar de novo -- duas vezes, no mesmo arquivo.
///
/// # Prova real
///
/// Devolver o `({e})` a este recado faz o teste cair em `nao pode mandar
/// reparar`: o texto cru volta, mandando reconstruir um indice intacto.
#[test]
fn a_procura_das_filhas_nao_manda_reparar_indice_sao() {
    let d = dir("filha-invisivel");
    let mut m = mae(&d);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();

    // NAO se insere a mae de destino aqui: `id` e unico, e uma linha com 7 ja
    // gravada faria o `atualizar` cair em «chave duplicada» ANTES de planejar
    // a cascata -- que foi o primeiro jeito que eu escrevi este teste, e ele
    // falhava pelo motivo errado. A recusa que se quer vem do PLANEJAMENTO.
    //
    // A filha fica com escrita PENDENTE: e o segundo descritor que
    // `planejar_ao_alterar` abre que ve a marca de sujo.
    let mut f = filha_com(&d, "pedidos", AcaoRi::Cascata, true, true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();

    let erro = m
        .atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect_err("a filha ainda nao esta visivel: tinha de recusar");
    let texto = erro.to_string();

    assert!(
        !texto.contains("reconstrua") && !texto.contains("reparar indice"),
        "nao pode mandar reparar: o arquivo esta sao, so nao esta sincronizado -- {texto}"
    );
    assert!(
        texto.contains("confirme-a antes de alterar a mae"),
        "tirou o texto cru mas perdeu a explicacao -- {texto}"
    );
    assert!(
        texto.contains(".ndx"),
        "perdeu a causa: o recado tem de dizer qual arquivo e qual guarda -- {texto}"
    );
}
