//! As peças do bidirecional que moram no STORE: o carimbo e a origem forçados
//! no evento, e a imagem que passa a viajar na exclusão física.
//!
//! O que estes testes protegem:
//!
//! 1. um evento APLICADO guarda o instante e o servidor em que a escrita
//!    nasceu — é o carimbo que decide o conflito «mais recente vence», e
//!    gravar a hora de chegada elegeria sempre quem sincroniza por último;
//! 2. o forçado vale UMA vez: a escrita local seguinte volta ao relógio local
//!    e à origem zero, senão um evento alheio contaminaria o diário inteiro;
//! 3. com `imagem_na_exclusao` ligada, o evento de exclusão carrega a linha —
//!    no bidirecional a identidade é a CHAVE, e a chave mora dentro da imagem.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn linha(i: i64, nome: &str) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(nome.into())]
}

#[test]
fn o_evento_forcado_guarda_carimbo_e_origem_do_nascimento() {
    let d = DirTemp::novo("forcado");
    let mut t = Table::criar(&d.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);

    // Um insert aplicado, vindo do servidor 0xCAFE, nascido em outro instante.
    t.forcar_proximo_evento(1_600_000_000_000, 0xCAFE);
    t.inserir(&linha(1, "aplicada")).unwrap();
    // E uma escrita LOCAL logo depois: o forçado não pode sobrar para ela.
    t.inserir(&linha(2, "local")).unwrap();

    let eventos = t.diario(0, 0).unwrap();
    assert_eq!(eventos[0].carimbo, 1_600_000_000_000);
    assert_eq!(eventos[0].origem, 0xCAFE);
    assert_eq!(eventos[1].origem, 0, "a escrita local e origem zero");
    assert!(
        eventos[1].carimbo > 1_700_000_000_000,
        "a escrita local volta ao relogio local"
    );
}

#[test]
fn o_forcado_vale_para_alteracao_e_exclusao_tambem() {
    let d = DirTemp::novo("forcado-3ops");
    let mut t = Table::criar(&d.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    t.inserir(&linha(1, "original")).unwrap();

    t.forcar_proximo_evento(1_600_000_000_001, 7);
    t.atualizar(1, &linha(1, "alterada")).unwrap();
    t.forcar_proximo_evento(1_600_000_000_002, 7);
    t.excluir_de_vez(1, "prova").unwrap();

    let eventos = t.diario(0, 0).unwrap();
    assert_eq!(eventos[1].operacao, Operacao::Alteracao);
    assert_eq!(
        (eventos[1].carimbo, eventos[1].origem),
        (1_600_000_000_001, 7)
    );
    assert_eq!(eventos[2].operacao, Operacao::Exclusao);
    assert_eq!(
        (eventos[2].carimbo, eventos[2].origem),
        (1_600_000_000_002, 7)
    );
}

#[test]
fn a_exclusao_carrega_a_imagem_quando_o_bidirecional_pede() {
    let d = DirTemp::novo("excl-imagem");
    let mut t = Table::criar(&d.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    t.ligar_imagem_na_exclusao(true);
    t.inserir(&linha(42, "vai embora")).unwrap();
    t.excluir_de_vez(1, "prova").unwrap();

    let eventos = t.diario_com_imagem(0, 0).unwrap();
    let (e, imagem) = &eventos[1];
    assert_eq!(e.operacao, Operacao::Exclusao);
    assert!(
        !imagem.is_empty(),
        "sem a imagem o outro lado nao sabe QUAL chave excluir"
    );
    // E a chave se le de dentro dela, pelo caminho publico.
    let valores = t.valores_da_imagem(imagem).unwrap();
    assert_eq!(valores[0], Value::Int(42));
}

#[test]
fn sem_o_interruptor_a_exclusao_continua_sem_imagem() {
    // O comportamento VELHO e o teste que mais importa: a replica classica
    // replica exclusao pelo rowid, e o evento dela nao pode inchar.
    let d = DirTemp::novo("excl-classica");
    let mut t = Table::criar(&d.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    t.inserir(&linha(1, "x")).unwrap();
    t.excluir_de_vez(1, "prova").unwrap();

    let eventos = t.diario_com_imagem(0, 0).unwrap();
    assert!(
        eventos[1].1.is_empty(),
        "a exclusao classica vai so com o rowid"
    );
}

// ---------------------------------------------------------------------------
// O bidirecional aplica sem julgar — e por que ele NÃO passa pelo
// `aplicar_evento`
// ---------------------------------------------------------------------------
//
// O bidirecional casa por CHAVE, não por rowid: o rowid e o rownum são locais,
// e a ordem de digitação de cada servidor é sagrada nele. Por isso ele chama o
// `inserir`/`atualizar`/`excluir_de_vez` de sempre — e caía no mesmo buraco da
// réplica: a chave estrangeira era conferida, o evento da filha que chegasse
// antes da mãe era recusado, o erro subia pelo `?` do laço, a posição nunca
// andava, e o mesmo lote voltava para sempre. Não é uma linha perdida: é o
// **par de servidores parado**.

use phxsql_core::schema::ForeignKey;

fn mae_bidi(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn filha_bidi(d: &std::path::Path) -> Table {
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
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(true)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A filha que chega antes da mãe ENTRA pelo caminho replicado, e é recusada
/// pelo caminho local. Os dois no mesmo teste, porque é a diferença entre eles
/// que é a garantia.
#[test]
fn o_bidirecional_aceita_a_filha_que_chega_antes_da_mae() {
    let d = DirTemp::novo("bidi-fk");
    mae_bidi(&d.0).sincronizar().unwrap();
    let mut f = filha_bidi(&d.0);

    // Local: recusa, e é isso que trava a origem antes de o evento nascer.
    f.inserir(&[Value::Int(10), Value::Int(1)])
        .expect_err("a escrita local confere");

    // Replicado: entra. O outro lado já aceitou.
    let rowid = f
        .inserir_replicado(&[Value::Int(10), Value::Int(1)])
        .expect("o bidirecional aplica o que a origem já aceitou");
    assert_eq!(rowid, 1);

    // E a marca não vaza: a escrita local seguinte volta a conferir.
    f.inserir(&[Value::Int(11), Value::Int(2)])
        .expect_err("a marca é de UMA escrita, não do handle");
}

/// A mãe sai pelo caminho replicado mesmo com filha viva: na origem a filha já
/// saiu primeiro (foi o `conferir_filhas` DELA que obrigou), e os dois eventos
/// chegam aqui em qualquer ordem.
#[test]
fn o_bidirecional_apaga_a_mae_cuja_filha_ainda_nao_saiu() {
    let d = DirTemp::novo("bidi-fk-del");
    let mut m = mae_bidi(&d.0);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();

    let mut f = filha_bidi(&d.0);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    // Local: recusa, e é a regra primordial fazendo o trabalho dela.
    m.sincronizar().unwrap();
    m.excluir_de_vez(1, "local")
        .expect_err("nunca se mata o pai que tem filhos");

    assert!(
        m.excluir_de_vez_replicado(1, "bidi").unwrap(),
        "o replicado aplica: a ordem se resolve no lote seguinte"
    );
    // E a marca não vaza. O `sincronizar` é obrigatório antes de a filha
    // procurar a mãe: um segundo descritor sobre tabela com escrita pendente
    // lê um índice que ainda não foi para o disco, e o store recusa — recusa
    // certo, e está no cabeçalho de `tests/chave-estrangeira.rs`.
    m.inserir(&[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let mut f = Table::abrir(&d.0, "pedidos").unwrap();
    f.inserir(&[Value::Int(11), Value::Int(2)])
        .expect("a mãe 2 está viva");
}

/// A troca de chave leva o «antes» no rabo da imagem, e a alteracao comum
/// nao leva nada. Medido pelo bidirecional em 01/10/2026: sem o «antes», o
/// outro lado procura a linha pela chave NOVA, nao acha, insere -- e a antiga
/// fica. A prova pelo soquete e `tests/identidade-do-bidirecional.rs` do
/// servidor; aqui e a peca do store.
///
/// **Defeito reposto**: `Table::muda_chave_unica` devolvendo sempre `false`
/// -- a primeira asercao cai, a imagem sai sem o rabo.
#[test]
fn a_troca_de_chave_leva_o_antes_e_a_alteracao_comum_nao() {
    let d = DirTemp::novo("troca-de-chave");
    let mut t = Table::criar(&d.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    t.inserir(&linha(1, "ana")).unwrap();
    t.atualizar(1, &linha(10, "ana")).unwrap();
    t.atualizar(1, &linha(10, "ana maria")).unwrap();

    let eventos = t.diario_com_imagem(0, 0).unwrap();
    let antes = t.valores_antes_da_imagem(&eventos[1].1).unwrap();
    assert_eq!(
        antes.map(|v| v[0].clone()),
        Some(Value::Int(1)),
        "a troca de chave saiu sem a chave de antes"
    );
    // A imagem continua dizendo o «depois», e a replica fiel le igual.
    assert_eq!(
        t.valores_da_imagem(&eventos[1].1).unwrap()[0],
        Value::Int(10)
    );
    // O comportamento VELHO: a alteracao que nao toca a chave nao paga um
    // byte, e a insercao nunca tem rabo.
    assert_eq!(t.valores_antes_da_imagem(&eventos[2].1).unwrap(), None);
    assert_eq!(Table::payload_antes_da_imagem(&eventos[0].1).unwrap(), None);
}

/// O decodificador da imagem RECUSA o que sobra e ele nao conhece -- parecer
/// do DBA de 01/10/2026, §2. Antes, um campo novo depois dos externos era
/// ignorado CALADO por todo binario anterior, e o par de versoes divergia sem
/// erro. O rabo do «antes», que e campo conhecido, continua aceito -- mas so
/// INTEIRO, fechando a imagem exato.
///
/// **Defeito reposto**: tirar o `Table::ler_o_rabo(imagem, i)?` do fim de
/// `desmontar_imagem` -- a primeira asercao cai, a sobra passa calada.
#[test]
fn a_imagem_com_sobra_desconhecida_e_recusada() {
    let d = DirTemp::novo("sobra-na-imagem");
    let mut t = Table::criar(&d.0, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    t.inserir(&linha(1, "ana")).unwrap();
    let limpa = t.imagem_da_linha_do_rowid(1).unwrap();

    let mut com_sobra = limpa.clone();
    com_sobra.push(0x01);
    let e = t
        .valores_da_imagem(&com_sobra)
        .expect_err("a sobra desconhecida passou calada");
    assert!(e.to_string().contains("atualize"), "{e}");

    // O rabo conhecido, inteiro: aceito.
    t.atualizar(1, &linha(2, "ana")).unwrap();
    let eventos = t.diario_com_imagem(0, 0).unwrap();
    let com_antes = eventos[1].1.clone();
    assert!(t.valores_da_imagem(&com_antes).is_ok());
    // E o rabo conhecido com um byte a mais depois dele: recusado tambem.
    let mut rabo_e_mais = com_antes;
    rabo_e_mais.push(0);
    assert!(t.valores_da_imagem(&rabo_e_mais).is_err());
    // A imagem sem rabo nenhum, a de sempre, continua lendo.
    assert_eq!(t.valores_da_imagem(&limpa).unwrap()[0], Value::Int(1));
}
