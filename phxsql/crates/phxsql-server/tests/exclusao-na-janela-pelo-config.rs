//! A ponta que liga o `config.json` ao `fsync` da exclusao.
//!
//! Arquivo proprio pela mesma razao do corte do diario e do da cifra:
//! `Recursos::aplicar` mexe num global do PROCESSO, e liga-lo dentro do
//! binario da biblioteca tiraria o `fsync` da exclusao de todo teste que
//! estivesse correndo ao lado.
//!
//! # O que este arquivo cobra
//!
//! 1. **config de ontem nao muda nada** -- e este e o teste que mais importa.
//!    O campo nasce desligado, e um `config.json` escrito antes dele continua
//!    com a garantia de sempre: um `excluir` que responde OK ja esta no disco;
//! 2. ligado, o valor CHEGA ao motor -- campo de configuracao que ninguem le
//!    e pior que campo ausente, e esta casa ja pagou isso tres vezes;
//! 3. o campo esta na lista unica que a tela monta (`CAMPOS_EDITAVEIS`), e nao
//!    numa segunda lista escrita no JavaScript.

use std::path::PathBuf;
use std::sync::Mutex;

use phxsql_server::config::{Config, TipoDoCampo, CAMPOS_EDITAVEIS};
use phxsql_store::lixeira;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn dir(rotulo: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-cfg-exc-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn escrever_config(d: &std::path::Path, conteudo: &str) -> PathBuf {
    let caminho = d.join("config.json");
    std::fs::write(&caminho, conteudo).unwrap();
    caminho
}

/// **O teste do comportamento VELHO, e ele e o portao desta frente.**
///
/// Reponha o defeito trocando o padrao de `exclusao_na_janela` para `true` em
/// `Recursos::default` -- que e exatamente como o Sprint 1 do
/// `SPRINTS-CASSANDRA.md` estava escrito antes de a §2.1 do `SPRINTS.md` o
/// reescrever -- e este teste cai.
#[test]
fn config_sem_o_campo_continua_esperando_o_disco() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    lixeira::definir_na_janela(false);
    let d = dir("padrao");
    let caminho = escrever_config(&d, r#"{"token":"t","recursos":{"threads":2}}"#);

    let c = Config::ler(&caminho).unwrap();
    assert!(
        !c.recursos.exclusao_na_janela,
        "a exclusao na janela nasceu ligada"
    );
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert!(
        !lixeira::na_janela(),
        "um config.json de ontem afrouxou a garantia da exclusao"
    );
}

/// Pedido, o campo chega ao motor -- e volta quando alguem o desliga.
///
/// Reponha o defeito tirando a linha `lixeira::definir_na_janela` de
/// `Recursos::aplicar`: o campo continua no arquivo, no MANUAL e na tela, e
/// nao faz nada. E a armadilha do `cache_paginas` prometendo um cache que nao
/// existia, e este teste cai nas duas assercoes do meio.
#[test]
fn pedido_no_config_o_valor_chega_ao_motor() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    lixeira::definir_na_janela(false);
    let d = dir("pedido");

    let caminho = escrever_config(
        &d,
        r#"{"token":"t","recursos":{"exclusao_na_janela":true}}"#,
    );
    let c = Config::ler(&caminho).unwrap();
    assert!(c.recursos.exclusao_na_janela);
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert!(lixeira::na_janela(), "o campo nao chegou ao motor");

    // E o caminho de volta, que e o que uma guarda afrouxada precisa ter:
    // desligar devolve a garantia sem reiniciar nada.
    let caminho = escrever_config(
        &d,
        r#"{"token":"t","recursos":{"exclusao_na_janela":false}}"#,
    );
    let c = Config::ler(&caminho).unwrap();
    assert!(!c.recursos.exclusao_na_janela);
    assert!(!lixeira::na_janela(), "desligar nao devolveu o fsync");
}

/// O campo aparece para quem edita pela tela, pelo ponto unico.
///
/// A tela monta o formulario da lista que o servidor manda; uma segunda lista
/// escrita no JavaScript envelheceria no primeiro campo acrescentado de um
/// lado so.
#[test]
fn o_campo_esta_na_lista_que_a_tela_monta() {
    let achado = CAMPOS_EDITAVEIS
        .iter()
        .find(|(campo, _, _)| *campo == "recursos.exclusao_na_janela");
    let (_, tipo, a_quente) = achado.expect("o campo nao chegou a tela");
    assert_eq!(*tipo, TipoDoCampo::Booleano);
    assert!(*a_quente, "o efeito e imediato: `aplicar` grava o global");

    let json = phxsql_server::config::editaveis_json().escrever();
    assert!(
        json.contains("recursos.exclusao_na_janela"),
        "a lista serializada nao leva o campo"
    );
}
