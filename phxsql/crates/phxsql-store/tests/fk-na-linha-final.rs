//! Pedido 514: a chave estrangeira se confere na linha FINAL -- a que vai ao
//! disco, depois do DEFAULT, da coluna calculada e da `Sequence` --, e nao na
//! linha crua que o cliente mandou.
//!
//! # O defeito que isto trava
//!
//! A conferencia rodava ANTES de o motor preencher a linha. O cliente mandava
//! `cod_cliente` nulo, nulo satisfaz chave estrangeira (`MATCH SIMPLE`), a
//! conferencia passava -- e so DEPOIS o DEFAULT punha 7 na coluna. A filha
//! nascia apontando para um cliente 7 que nunca existiu, fora e dentro de
//! transacao. A coluna calculada caia pelo mesmo buraco, e na alteracao
//! tambem. Fere a regra primordial («so existe filho se o pai existir») pelo
//! lado que ninguem olhava: o valor que o PROPRIO motor escreveu.
//!
//! PostgreSQL, MySQL e MariaDB conferem a linha final (a FK do PG e um gatilho
//! `AFTER ROW` sobre a tupla gravada; InnoDB confere no `row_ins_check_foreign
//! _constraints`, com a linha ja montada): aceite automatico.
//!
//! # Os controles
//!
//! Cada recusa tem o seu positivo ao lado: DEFAULT com mae existente grava, e
//! valor explicito continua mandando sobre o DEFAULT. Sem eles, um portao que
//! recusasse toda linha com DEFAULT passaria por estes testes.

mod comum;
use phxsql_core::error::PhxError;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{MaesEmProgresso, Pendente, Table, Visao};

fn dir(nome: &str) -> comum::DirTemp {
    comum::DirTemp::novo(&format!("fk-final-514-{nome}"))
}

/// `clientes(id)`, com as ids pedidas ja gravadas e sincronizadas -- a
/// conferencia abre a mae por um segundo descritor, que so ve o gravado.
fn clientes(d: &std::path::Path, ids: &[i64]) {
    let e = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut m = Table::criar(d, e).unwrap();
    for &id in ids {
        m.inserir(&[Value::Int(id)]).unwrap();
    }
    m.sincronizar().unwrap();
}

fn fk() -> ForeignKey {
    ForeignKey::new("fk_cliente", vec![1], "clientes", vec!["id".into()]).conferindo(true)
}

/// `pedidos(id, cod_cliente DEFAULT 7)` -- o caso do parecer, N1.
fn pedidos(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("cod_cliente", ColumnType::Int8)
                .com_padrao("7")
                .unwrap(),
        ],
        vec![
            IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("por_cliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![fk()])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// `itens(id, cod_cliente = x + 0, x)` -- a calculada do parecer, N1.
fn itens(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "itens",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("cod_cliente", ColumnType::Int8)
                .com_calculada("x + 0")
                .unwrap(),
            Column::new("x", ColumnType::Int8),
        ],
        vec![
            IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("por_cliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![fk()])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A recusa tem de ser a da CHAVE, nomeando a mae -- e nao um CHECK, uma
/// aridade ou um erro de tipo que passasse por ela.
fn e_recusa_da_fk(r: Result<impl std::fmt::Debug, PhxError>, contexto: &str) {
    match r {
        Err(PhxError::Integridade(m)) if m.contains("fk_cliente") && m.contains("clientes") => {}
        outro => panic!("{contexto}: esperava a recusa da fk_cliente, veio {outro:?}"),
    }
}

fn vivas(t: &mut Table) -> u64 {
    t.contar(Visao::Todas).unwrap()
}

/// Nenhuma mae aberta pela transacao: a conferencia vai ao disco, como fora
/// dela. E o papel do mapa vazio da pre-conferencia.
struct SemMaes;

impl MaesEmProgresso for SemMaes {
    fn mae(&mut self, _tabela_ref: &str) -> Option<&mut Table> {
        None
    }
}

// ------------------------------------------------------------------ DEFAULT

/// **O caso do parecer:** `cod_cliente` nulo, DEFAULT 7, e so o cliente 1
/// existe. Antes do conserto a linha GRAVAVA com 7.
#[test]
fn o_default_sem_mae_e_recusado_no_inserir() {
    let d = dir("default-sem-mae");
    clientes(&d, &[1]);
    let mut p = pedidos(&d);
    e_recusa_da_fk(
        p.inserir(&[Value::Int(1), Value::Null]),
        "o DEFAULT 7 apontou para um cliente que nao existe",
    );
    assert_eq!(vivas(&mut p), 0, "a filha orfa foi gravada");
}

/// O controle: com o cliente 7 no disco, o mesmo pedido grava -- e grava com
/// o 7 que o DEFAULT pos. E o valor explicito continua mandando: `1` passa.
#[test]
fn o_default_com_mae_grava_e_o_valor_explicito_continua_mandando() {
    let d = dir("default-com-mae");
    clientes(&d, &[1, 7]);
    let mut p = pedidos(&d);
    let r = p
        .inserir(&[Value::Int(1), Value::Null])
        .expect("o DEFAULT 7 com o cliente 7 existente foi recusado");
    assert_eq!(p.ler(r).unwrap().unwrap()[1], Value::Int(7));
    let r = p
        .inserir(&[Value::Int(2), Value::Int(1)])
        .expect("o valor explicito com mae existente foi recusado");
    assert_eq!(p.ler(r).unwrap().unwrap()[1], Value::Int(1));
}

/// O lote chama o `inserir` por linha, e a linha recusada sai com a posicao
/// -- as outras duas, com mae, gravam.
#[test]
fn o_lote_confere_a_linha_final() {
    let d = dir("lote");
    clientes(&d, &[1]);
    let mut p = pedidos(&d);
    let lote = p
        .inserir_lote(
            &[
                vec![Value::Int(1), Value::Int(1)],
                vec![Value::Int(2), Value::Null],
                vec![Value::Int(3), Value::Int(1)],
            ],
            false,
        )
        .unwrap();
    assert_eq!(lote.rowids.len(), 2, "{:?}", lote.recusadas);
    assert_eq!(
        lote.recusadas.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![1],
        "a linha do DEFAULT sem mae nao foi a recusada: {:?}",
        lote.recusadas
    );
}

// ---------------------------------------------------------------- calculada

/// **A calculada do parecer:** `cod_cliente = x + 0`. Inserir com `x = 9`,
/// sem cliente 9, gravava; alterar para `x = 8`, sem cliente 8, tambem.
#[test]
fn a_calculada_sem_mae_e_recusada_no_inserir_e_no_atualizar() {
    let d = dir("calculada");
    clientes(&d, &[9]);
    let mut t = itens(&d);
    e_recusa_da_fk(
        t.inserir(&[Value::Int(1), Value::Null, Value::Int(8)]),
        "a calculada 8 apontou para um cliente que nao existe",
    );
    assert_eq!(vivas(&mut t), 0, "o item orfao foi gravado");

    // O controle: com mae, a calculada grava -- e grava o valor dela.
    let r = t
        .inserir(&[Value::Int(1), Value::Null, Value::Int(9)])
        .expect("a calculada 9 com o cliente 9 existente foi recusada");
    let mut linha = t.ler(r).unwrap().unwrap();
    assert_eq!(linha[1], Value::Int(9));

    // A alteracao: o cliente manda a linha que leu, com o 9 antigo na
    // calculada, e muda so o `x`. A linha final aponta para 8.
    linha[2] = Value::Int(8);
    e_recusa_da_fk(
        t.atualizar(r, &linha),
        "a alteracao levou a calculada a um cliente que nao existe",
    );
    assert_eq!(
        t.ler(r).unwrap().unwrap()[1],
        Value::Int(9),
        "a alteracao recusada mudou a linha"
    );
}

// ------------------------------------------------------- `Sequence` como FK

/// A tabela 1-para-1 cuja primaria `Sequence` tambem aponta para a mae: o
/// nulo que chega vira o proximo numero, e o numero e que tem de existir la.
/// E a «qualquer outra transformacao» do contrato -- a linha final e a que
/// vai ao disco, venha o valor de onde vier.
#[test]
fn a_sequence_que_e_chave_estrangeira_confere_o_numero_gerado() {
    let d = dir("sequence");
    clientes(&d, &[1]);
    let e = Schema::new(
        "detalhe",
        vec![
            Column::new("id", ColumnType::Sequence).obrigatoria(),
            Column::new("nota", ColumnType::Int8),
        ],
        vec![IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![0],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(true)])
    .unwrap();
    let mut t = Table::criar(&d, e).unwrap();
    t.inserir(&[Value::Null, Value::Int(10)])
        .expect("o numero 1 tem mae, e foi recusado");
    e_recusa_da_fk(
        t.inserir(&[Value::Null, Value::Int(20)]),
        "a Sequence gerou 2 e o cliente 2 nao existe",
    );
    assert_eq!(vivas(&mut t), 1, "o detalhe orfao foi gravado");
}

// ------------------------------------------- a pre-conferencia do COMMIT (448)

/// A pre-conferencia do 448 herdava a mesma ordem: conferia a linha crua e
/// so depois previa a completada. Aprovava a filha orfa, e a passada -- que
/// depois do conserto confere a final -- recusaria DEPOIS da marca.
#[test]
fn a_pre_conferencia_confere_a_linha_prevista() {
    let d = dir("pre-conferencia");
    clientes(&d, &[1, 9]);
    let mut p = pedidos(&d);
    p.sincronizar().unwrap();
    e_recusa_da_fk(
        p.pre_conferir(
            1,
            Pendente::Insercao(&[Value::Int(1), Value::Null]),
            "",
            &mut SemMaes,
        ),
        "a pre-conferencia aprovou o DEFAULT sem mae",
    );
    // O controle, na mesma tabela: o valor explicito com mae passa.
    p.pre_conferir(
        1,
        Pendente::Insercao(&[Value::Int(1), Value::Int(1)]),
        "",
        &mut SemMaes,
    )
    .expect("a pre-conferencia recusou a linha com mae");

    let mut t = itens(&d);
    let r = t
        .inserir(&[Value::Int(1), Value::Null, Value::Int(9)])
        .unwrap();
    t.sincronizar().unwrap();
    let mut linha = t.ler(r).unwrap().unwrap();
    linha[2] = Value::Int(8);
    e_recusa_da_fk(
        t.pre_conferir(r, Pendente::Alteracao(&linha, &[]), "", &mut SemMaes),
        "a pre-conferencia aprovou a alteracao da calculada sem mae",
    );
}

// ------------------------------------------ a cascata do `ao_alterar` (P1)

/// `clientes(id, codigo)` com `codigo` unico -- a mae que troca de CHAVE sem
/// trocar de `id`, que e o caso em que a cascata anda.
fn clientes_por_codigo(d: &std::path::Path, codigos: &[i64]) {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("codigo", ColumnType::Int8),
        ],
        vec![
            IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("por_codigo", vec![IndexColumn::asc(1)]).unico(),
        ],
    )
    .unwrap();
    let mut m = Table::criar(d, e).unwrap();
    for (i, &c) in codigos.iter().enumerate() {
        m.inserir(&[Value::Int(i as i64 + 1), Value::Int(c)])
            .unwrap();
    }
    m.sincronizar().unwrap();
}

/// A filha com a coluna da chave declarada como o teste pedir, e a chave em
/// CASCATA -- montada pela API do `Schema`, que e o caminho de LER: e assim
/// que uma tabela criada antes da recusa na declaracao volta do disco.
fn filha_em_cascata(d: &std::path::Path, coluna: Column) -> Table {
    let e = Schema::new(
        "itens",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            coluna,
            Column::new("x", ColumnType::Int8),
        ],
        vec![
            IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("por_cliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["codigo".into()],
    )
    .ao_alterar(AcaoRi::Cascata)
    .conferindo(true)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn cod_calculado() -> Column {
    Column::new("cod_cliente", ColumnType::Int8)
        .com_calculada("x + 0")
        .unwrap()
}

fn cod_com_check() -> Column {
    Column::new("cod_cliente", ColumnType::Int8)
        .com_check("cod_cliente < 100")
        .unwrap()
}

/// O `codigo` da mae e a chave da filha, relidos por handles NOVOS -- o que
/// ficou no disco, e nao o que um handle aberto acha que gravou.
fn no_disco(d: &std::path::Path) -> (Value, Value) {
    let mut m = Table::abrir(d, "clientes").unwrap();
    let mut f = Table::abrir(d, "itens").unwrap();
    (
        m.ler(1).unwrap().unwrap()[1].clone(),
        f.ler(1).unwrap().unwrap()[1].clone(),
    )
}

/// A mae 1 muda o `codigo` para `novo`, por um handle proprio.
fn mae_muda_para(d: &std::path::Path, novo: i64) -> phxsql_core::error::Result<()> {
    let mut m = Table::abrir(d, "clientes").unwrap();
    m.atualizar(1, &[Value::Int(1), Value::Int(novo)])
}

/// **P1, o irmao do 514:** a chave da filha e CALCULADA (`x + 0`) e declara
/// cascata. A mae troca de 9 para 8: a cascata levaria o 8 ate a filha, e a
/// calculada o desfaz para 9. No `9c4bbd0` a resposta era OK e a filha ficava
/// orfa calada; depois do 514, ERRO -- mas com a mae ja gravada em 8. Fora de
/// transacao, a recusa tem de vir ANTES da primeira escrita: mae em 9, zero
/// gravado.
#[test]
fn a_cascata_sobre_calculada_recusa_antes_de_gravar_a_mae() {
    let d = dir("cascata-calculada");
    clientes_por_codigo(&d, &[9]);
    let mut f = filha_em_cascata(&d, cod_calculado());
    f.inserir(&[Value::Int(1), Value::Null, Value::Int(9)])
        .unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = mae_muda_para(&d, 8);
    let disco = no_disco(&d);
    match r {
        Err(PhxError::Integridade(t)) if t.contains("cod_cliente") && t.contains("calculada") => {}
        outro => panic!(
            "a cascata sobre a calculada nao recusou antes de gravar: {outro:?} -- \
             (mae, filha) no disco {disco:?}"
        ),
    }
    assert_eq!(
        disco,
        (Value::Int(9), Value::Int(9)),
        "(mae, filha): a recusa deixou coisa gravada"
    );
}

/// **O irmao pelo CHECK da filha:** a coluna da chave e comum, mas a filha
/// declara `cod_cliente < 100`. A mae vai de 9 para 200, e a cascata poria 200
/// numa linha que o CHECK recusa -- a mesma recusa depois da mae gravada, pelo
/// mesmo motivo: o elo nao era conferido na linha final da filha.
#[test]
fn o_check_da_filha_recusa_a_cascata_antes_de_gravar_a_mae() {
    let d = dir("cascata-check");
    clientes_por_codigo(&d, &[9]);
    let mut f = filha_em_cascata(&d, cod_com_check());
    f.inserir(&[Value::Int(1), Value::Int(9), Value::Int(0)])
        .unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = mae_muda_para(&d, 200);
    let disco = no_disco(&d);
    match r {
        Err(PhxError::Integridade(t)) if t.contains("CHECK") && t.contains("Nada foi gravado") => {}
        outro => panic!(
            "o CHECK da filha nao recusou a cascata antes de gravar: {outro:?} -- \
             (mae, filha) no disco {disco:?}"
        ),
    }
    assert_eq!(disco, (Value::Int(9), Value::Int(9)));
}

/// O controle, pelo mesmo caminho novo: a filha TEM regra (o CHECK passa com
/// o valor novo), entao o elo e conferido na linha final -- e a cascata anda.
/// Sem ele, uma conferencia que recusasse toda filha com regra passaria pelos
/// dois de cima.
#[test]
fn a_cascata_sobre_coluna_comum_com_regra_continua_levando_a_filha() {
    let d = dir("cascata-comum");
    clientes_por_codigo(&d, &[9]);
    let mut f = filha_em_cascata(&d, cod_com_check());
    f.inserir(&[Value::Int(1), Value::Int(9), Value::Int(0)])
        .unwrap();
    f.sincronizar().unwrap();
    drop(f);

    mae_muda_para(&d, 8).expect("a cascata legitima foi recusada");
    assert_eq!(no_disco(&d), (Value::Int(8), Value::Int(8)));
}

/// **O banco que ja existe:** a tabela nascida com o par (chave calculada em
/// cascata) antes da recusa na declaracao continua abrindo, lendo e gravando.
/// A guarda nova entra pedida: o que ela muda e so a alteracao da mae, que
/// passa a recusar inteira em vez de pela metade.
#[test]
fn a_tabela_velha_com_o_par_continua_abrindo_e_gravando() {
    let d = dir("par-velho");
    clientes_por_codigo(&d, &[9, 7]);
    let mut f = filha_em_cascata(&d, cod_calculado());
    f.inserir(&[Value::Int(1), Value::Null, Value::Int(9)])
        .unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let mut f = Table::abrir(&d, "itens").expect("a tabela velha com o par nao abriu");
    assert_eq!(f.ler(1).unwrap().unwrap()[1], Value::Int(9));
    let r = f
        .inserir(&[Value::Int(2), Value::Null, Value::Int(7)])
        .expect("a tabela velha com o par nao grava mais");
    assert_eq!(f.ler(r).unwrap().unwrap()[1], Value::Int(7));
    let mut linha = f.ler(1).unwrap().unwrap();
    linha[2] = Value::Int(7);
    f.atualizar(1, &linha)
        .expect("a alteracao da filha com mae existente foi recusada");
}
