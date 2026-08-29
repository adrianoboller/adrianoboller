//! A janela de conflito de escrita: a versao por registro do `.reg`.
//!
//! O caso que estes testes protegem e o de duas pessoas com a mesma ficha
//! aberta. Sem a guarda, a segunda gravacao apaga o trabalho da primeira sem
//! erro e sem registro -- o pior tipo de perda, porque nao aparece em lugar
//! nenhum ate faltar o dado.
//!
//! O que se confere aqui:
//!
//! 1. a versao nasce em 1 e sobe a cada regravacao;
//! 2. gravar com a versao lida passa; com uma versao velha, recusa;
//! 3. a recusa e `Conflito` (3004) e **nao grava nada**;
//! 4. a exclusao suave conta como alteracao -- ela regrava o slot;
//! 5. excluida de vez tambem e conflito, e nao "nao encontrado";
//! 6. quem rele e regrava consegue passar.

#[allow(
    dead_code,
    reason = "o modulo comum serve a varios testes; este usa so o DirTemp"
)]
mod comum;

use comum::DirTemp;

use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const ID: usize = 0;
const NOME: usize = 1;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(ID)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn cliente(id: i64, nome: &str, cidade: &str) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(nome.into()),
        Value::Str(cidade.into()),
        Value::Bool(false),
        Value::UInt(0),
    ]
}

fn com_uma_linha(dir: &DirTemp) -> (Table, u64) {
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    let r = t.inserir(&cliente(1, "Adriano", "Blumenau")).unwrap();
    (t, r)
}

#[test]
fn a_versao_nasce_em_um() {
    let dir = DirTemp::novo("conflito-nasce");
    let (mut t, r) = com_uma_linha(&dir);
    assert_eq!(t.versao(r).unwrap(), Some(1));
}

#[test]
fn a_versao_sobe_a_cada_gravacao() {
    let dir = DirTemp::novo("conflito-sobe");
    let (mut t, r) = com_uma_linha(&dir);
    for esperada in 2..=5u64 {
        t.atualizar(r, &cliente(1, "Adriano", "Joinville")).unwrap();
        assert_eq!(t.versao(r).unwrap(), Some(esperada));
    }
}

#[test]
fn gravar_com_a_versao_lida_passa() {
    let dir = DirTemp::novo("conflito-passa");
    let (mut t, r) = com_uma_linha(&dir);
    let lida = t.versao(r).unwrap().unwrap();
    t.atualizar_se(r, &cliente(1, "Adriano", "Curitiba"), lida)
        .unwrap();
    assert_eq!(t.versao(r).unwrap(), Some(2));
}

/// O caso do enunciado: os dois leem a versao 1, e o segundo perde.
#[test]
fn gravar_com_versao_velha_recusa() {
    let dir = DirTemp::novo("conflito-recusa");
    let (mut t, r) = com_uma_linha(&dir);
    let lida_pelos_dois = t.versao(r).unwrap().unwrap();

    // O primeiro grava e vai embora.
    t.atualizar_se(r, &cliente(1, "Adriano", "Curitiba"), lida_pelos_dois)
        .unwrap();

    // O segundo chega com a versao que leu la atras.
    let e = t
        .atualizar_se(r, &cliente(1, "Adriano", "Recife"), lida_pelos_dois)
        .unwrap_err();
    assert!(matches!(e, PhxError::Conflito(_)), "veio {e:?}");
    assert_eq!(e.codigo(), 3004);
    assert_eq!(e.nome(), "CONFLITO");
    // Repetir sozinho seria escrever por cima do outro sem olhar.
    assert!(!e.adianta_repetir());
}

/// Recusar pela metade seria pior do que nao recusar: a guarda so vale se
/// **nada** tiver sido gravado quando ela dispara.
#[test]
fn o_conflito_nao_grava_nada() {
    let dir = DirTemp::novo("conflito-intacto");
    let (mut t, r) = com_uma_linha(&dir);
    let velha = t.versao(r).unwrap().unwrap();
    t.atualizar_se(r, &cliente(1, "Adriano", "Curitiba"), velha)
        .unwrap();

    let _ = t.atualizar_se(r, &cliente(1, "OUTRO", "Recife"), velha);

    let linha = t.ler(r).unwrap().unwrap();
    assert_eq!(linha[NOME], Value::Str("Adriano".into()));
    assert_eq!(linha[2], Value::Str("Curitiba".into()));
    assert_eq!(
        t.versao(r).unwrap(),
        Some(2),
        "a versao nao pode ter subido"
    );
}

/// Reler e regravar e o caminho de saida -- e ele tem de funcionar, senao a
/// guarda vira um beco sem saida.
#[test]
fn quem_rele_consegue_gravar() {
    let dir = DirTemp::novo("conflito-rele");
    let (mut t, r) = com_uma_linha(&dir);
    let velha = t.versao(r).unwrap().unwrap();
    t.atualizar(r, &cliente(1, "Adriano", "Curitiba")).unwrap();
    assert!(t
        .atualizar_se(r, &cliente(1, "Adriano", "Recife"), velha)
        .is_err());

    let agora = t.versao(r).unwrap().unwrap();
    t.atualizar_se(r, &cliente(1, "Adriano", "Recife"), agora)
        .unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[2], Value::Str("Recife".into()));
    assert_eq!(t.versao(r).unwrap(), Some(3));
}

/// Marcar como excluida REGRAVA o slot -- entao ela conta como alteracao, e
/// quem leu antes nao grava por cima sem ver que a linha foi excluida.
#[test]
fn a_exclusao_suave_conta_como_alteracao() {
    let dir = DirTemp::novo("conflito-suave");
    let (mut t, r) = com_uma_linha(&dir);
    let antes = t.versao(r).unwrap().unwrap();
    assert!(t.excluir_suave(r, "saiu da carteira").unwrap());
    assert_eq!(t.versao(r).unwrap(), Some(antes + 1));

    let e = t
        .atualizar_se(r, &cliente(1, "Adriano", "Recife"), antes)
        .unwrap_err();
    assert!(matches!(e, PhxError::Conflito(_)), "veio {e:?}");
}

/// Excluida de vez e conflito, e nao "nao encontrado": quem leu a linha ha um
/// minuto precisa saber que ela foi APAGADA, e nao que o rowid nunca existiu.
#[test]
fn excluida_de_vez_e_conflito() {
    let dir = DirTemp::novo("conflito-fisica");
    let (mut t, r) = com_uma_linha(&dir);
    let lida = t.versao(r).unwrap().unwrap();
    assert!(t.excluir_de_vez(r, "duplicada").unwrap());

    assert_eq!(t.versao(r).unwrap(), None);
    let e = t
        .atualizar_se(r, &cliente(1, "Adriano", "Recife"), lida)
        .unwrap_err();
    assert!(matches!(e, PhxError::Conflito(_)), "veio {e:?}");
    assert!(
        e.to_string().contains("excluido de vez"),
        "a mensagem tem de dizer o que houve: {e}"
    );
}

/// Rowid fora da tabela ERRA -- nao devolve "sem versao". A diferenca
/// importa: `None` quer dizer "esta linha nao existe mais", e um rowid
/// inventado nao pode se passar por linha excluida.
#[test]
fn rowid_fora_da_faixa_erra() {
    let dir = DirTemp::novo("conflito-faixa");
    let (mut t, r) = com_uma_linha(&dir);
    assert!(t.versao(r + 10_000).is_err());
}

/// A guarda so olha o registro pedido: alterar o vizinho nao pode invalidar a
/// versao que ninguem tocou.
#[test]
fn a_versao_e_de_cada_registro() {
    let dir = DirTemp::novo("conflito-vizinho");
    let mut t = Table::criar(&dir.0, esquema()).unwrap();
    let a = t.inserir(&cliente(1, "Adriano", "Blumenau")).unwrap();
    let b = t.inserir(&cliente(2, "Maria", "Blumenau")).unwrap();
    let versao_b = t.versao(b).unwrap().unwrap();

    for _ in 0..3 {
        t.atualizar(a, &cliente(1, "Adriano", "Joinville")).unwrap();
    }
    assert_eq!(t.versao(b).unwrap(), Some(versao_b));
    t.atualizar_se(b, &cliente(2, "Maria", "Itajai"), versao_b)
        .unwrap();
}
