//! A irma que nao abre, vista pela busca reversa da integridade -- pedido 631
//! (revisao SEC de 01/10/2026, M3).
//!
//! A busca reversa pergunta «alguem aponta para mim?», e a irma cujo `.reg`
//! nao abre e justamente a que nao responde. Ate aqui ela ficava de fora
//! (`Err(_) => continue`), e pular era responder «nao» por ela: com o
//! cabecalho da filha truncado, a mae excluia a linha que tinha filha. Falha
//! ABERTA na regra primordial.
//!
//! Os quatro caminhos que fazem a pergunta -- o excluir de vez, o suave, o
//! `ao_alterar` e o `excluir_tabela` -- passam pelo MESMO
//! `irmas::abrir_irma`, e cada um tem aqui a sua prova. E o comportamento
//! velho tem a dele: irma legivel e sem chave para a mae nao muda nada, e a
//! irma quebrada continua saindo pelo `excluir_tabela` dela.

mod comum;
use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::{Database, Instancia};
use phxsql_store::table::Table;

fn mae() -> Schema {
    Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn filha() -> Schema {
    Schema::new(
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
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )])
    .unwrap()
}

/// Uma irma legivel e SEM chave nenhuma -- o comportamento velho.
fn notas() -> Schema {
    Schema::new(
        "notas",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// A base `loja` com a mae (linhas 1 e 2) e a filha apontando para a 1.
fn base(rotulo: &str) -> (Instancia, Database, comum::DirTemp, Table) {
    let d = comum::DirTemp::novo(&format!("irma-nao-abre-{rotulo}"));
    let cat = Instancia::nova(&d).unwrap();
    let db = cat.criar_database("loja").unwrap();
    let mut m = db.criar_tabela(None, mae()).unwrap();
    m.inserir(&[Value::Int(1)]).unwrap();
    m.inserir(&[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = db.criar_tabela(None, filha()).unwrap();
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    (cat, db, d, m)
}

/// Trunca o cabecalho do `.reg` da filha: o magic fica, o resto some. A
/// abertura recusa por «truncado» -- e a filha continua la, com a linha que
/// aponta para a mae 1.
fn quebrar_a_filha(d: &comum::DirTemp) {
    let reg = d.join("loja").join("pedidos.reg");
    let f = std::fs::OpenOptions::new().write(true).open(&reg).unwrap();
    f.set_len(16).unwrap();
    f.sync_all().unwrap();
    assert!(
        Table::abrir(d.join("loja"), "pedidos").is_err(),
        "a filha truncada ainda abre: o teste nao prova o caso"
    );
}

fn recusa_nomeando_a_filha(e: PhxError, caminho: &str) {
    let txt = e.to_string();
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "{caminho}: familia errada: {txt}"
    );
    assert!(
        txt.contains("pedidos") && txt.contains("nao abre"),
        "{caminho}: a recusa nao diz QUAL irma nem por que: {txt}"
    );
}

/// **Prova real (pedido 631):** com o `.reg` da filha truncado, a mae com
/// filha NAO sai -- nem de vez, nem suave.
///
/// Reponha o defeito (`Err(_) => continue` no `irmas::chaves_das_irmas`): as
/// duas exclusoes passam e os `expect_err` caem.
#[test]
fn a_filha_que_nao_abre_tranca_o_excluir_de_vez_e_o_suave() {
    let (_cat, _db, d, mut m) = base("excluir");
    quebrar_a_filha(&d);
    let e = m
        .excluir_de_vez(1, "tentando")
        .expect_err("a mae com filha ilegivel foi apagada -- a regra primordial caiu");
    recusa_nomeando_a_filha(e, "excluir_de_vez");
    let e = m
        .excluir_suave(1, "tentando")
        .expect_err("a mae com filha ilegivel foi marcada excluida");
    recusa_nomeando_a_filha(e, "excluir_suave");
    assert_eq!(m.registros(), 2, "a mae perdeu linha");
}

/// O IRMAO no catalogo: apagar a tabela mae com a filha ilegivel recusa.
///
/// Reponha o defeito (`let Ok(reg) = .. else { continue }` no
/// `quem_aponta_para`): a mae some com a filha apontando para ela.
#[test]
fn a_filha_que_nao_abre_tranca_o_excluir_tabela_da_mae() {
    let (_cat, db, d, m) = base("excluir-tabela");
    drop(m);
    quebrar_a_filha(&d);
    let e = db
        .excluir_tabela("clientes")
        .expect_err("a tabela mae foi apagada com a filha ilegivel");
    recusa_nomeando_a_filha(e, "excluir_tabela");
    assert!(db.existe_tabela(None, "clientes").unwrap());
}

/// O IRMAO do `ao_alterar`: mudar a chave da mae com a filha ilegivel recusa
/// -- a cascata que tinha de levar a filha nao a alcanca.
///
/// Reponha o defeito (`Err(_) => continue` no `planejar_ao_alterar_com`): a
/// mae 1 vira 5 e a filha fica apontando para a 1, que nao existe mais.
#[test]
fn a_filha_que_nao_abre_tranca_a_alteracao_da_chave_da_mae() {
    let (_cat, _db, d, mut m) = base("alterar");
    quebrar_a_filha(&d);
    let e = m
        .atualizar(1, &[Value::Int(5)])
        .expect_err("a chave da mae mudou com a filha ilegivel");
    recusa_nomeando_a_filha(e, "atualizar");
}

/// **O comportamento VELHO:** irma legivel e sem chave para a mae nao muda
/// nada -- a linha sem filha sai, de vez e suave, e a tabela sai inteira.
/// Sem este, a prova de cima passaria com um portao que recusa tudo.
#[test]
fn irma_legivel_sem_chave_nao_muda_nada() {
    let d = comum::DirTemp::novo("irma-nao-abre-velho");
    let cat = Instancia::nova(&d).unwrap();
    let db = cat.criar_database("loja").unwrap();
    let mut m = db.criar_tabela(None, mae()).unwrap();
    for i in 1..=3 {
        m.inserir(&[Value::Int(i)]).unwrap();
    }
    m.sincronizar().unwrap();
    let mut n = db.criar_tabela(None, notas()).unwrap();
    n.inserir(&[Value::Int(1)]).unwrap();
    n.sincronizar().unwrap();
    drop(n);
    assert!(m.excluir_de_vez(1, "sem filha").unwrap());
    assert!(m.excluir_suave(2, "sem filha").unwrap());
    m.atualizar(3, &[Value::Int(30)]).unwrap();
    drop(m);
    db.excluir_tabela("clientes").unwrap();
}

/// **A porta de saida:** a irma quebrada sai pelo `excluir_tabela` DELA, que
/// pula a propria tabela na busca -- e entao a mae volta a excluir. Sem esta
/// porta a recusa seria eterna.
#[test]
fn a_filha_quebrada_sai_pelo_excluir_tabela_dela_e_a_mae_volta_a_excluir() {
    let (_cat, db, d, mut m) = base("saida");
    quebrar_a_filha(&d);
    assert!(m.excluir_de_vez(1, "tentando").is_err());
    db.excluir_tabela("pedidos").unwrap();
    assert!(m.excluir_de_vez(1, "a filha foi apagada").unwrap());
}
