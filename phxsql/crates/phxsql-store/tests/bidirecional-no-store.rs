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
