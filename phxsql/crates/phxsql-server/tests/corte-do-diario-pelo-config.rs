//! A ponta que liga o `config.json` ao corte de volume do diario.
//!
//! Arquivo proprio pela mesma razao do da cifra: `Recursos::aplicar` mexe num
//! global do processo, e um global mexido no binario da biblioteca faria o
//! `.log` de outro teste virar de volume no meio da corrida.

use std::path::PathBuf;
use std::sync::Mutex;

use phxsql_core::paginacao::Paginacao;
use phxsql_server::config::Config;
use phxsql_store::diario;
use phxsql_store::log::{LogFile, Operacao};

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn dir(rotulo: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-cfg-corte-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn escrever_config(d: &std::path::Path, conteudo: &str) -> PathBuf {
    let caminho = d.join("config.json");
    std::fs::write(&caminho, conteudo).unwrap();
    caminho
}

/// **O teste que mais importa: config de ontem nao muda nada.**
///
/// Sem `recursos.diario_volume_mib`, o corte continua sendo o do esquema -- e
/// um `.log` de 20.000 eventos continua num volume so, exatamente como antes.
#[test]
fn config_sem_o_campo_nao_muda_o_corte() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    diario::definir_bytes_por_volume(0);
    let d = dir("padrao");
    let caminho = escrever_config(&d, r#"{"token":"t","recursos":{"threads":2}}"#);

    let c = Config::ler(&caminho).unwrap();
    assert_eq!(c.recursos.diario_volume_mib, 0);
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert_eq!(
        diario::bytes_por_volume(),
        0,
        "um config velho mexeu no corte"
    );

    let mut l = LogFile::criar(&d, "t", Paginacao::nova(1_000_000, 99).unwrap()).unwrap();
    for i in 1..=20_000u64 {
        l.registrar(Operacao::Inclusao, i, 1).unwrap();
    }
    l.sincronizar().unwrap();
    assert_eq!(l.volumes().len(), 1, "o diario virou de volume sozinho");
    std::fs::remove_dir_all(&d).unwrap();
}

/// O campo e lido por alguma linha de codigo -- e a prova e o arquivo em disco.
///
/// E a pergunta que o projeto ja pagou caro para aprender a fazer:
/// `recursos.cache_paginas` passou tres versoes no `config.json`, no MANUAL e
/// na tela sem que uma unica linha o lesse.
#[test]
fn o_campo_do_config_corta_o_diario_de_verdade() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    diario::definir_bytes_por_volume(0);
    let d = dir("corta");
    let caminho = escrever_config(&d, r#"{"token":"t","recursos":{"diario_volume_mib":1}}"#);

    let c = Config::ler(&caminho).unwrap();
    assert_eq!(c.recursos.diario_volume_mib, 1);
    assert_eq!(
        diario::bytes_por_volume(),
        1024 * 1024,
        "o config.json pediu 1 MiB e nenhuma linha de codigo leu o campo"
    );

    let mut l = LogFile::criar(&d, "t", Paginacao::nova(1_000_000, 99).unwrap()).unwrap();
    // 30.000 eventos de 44 bytes dao 1,26 MiB: passa de um volume de 1 MiB.
    for i in 1..=30_000u64 {
        l.registrar(Operacao::Inclusao, i, 1).unwrap();
    }
    l.sincronizar().unwrap();
    assert!(
        l.volumes().len() > 1,
        "o corte de 1 MiB nao fechou volume nenhum"
    );
    assert_eq!(l.total().unwrap(), 30_000);
    assert_eq!(l.verificar().unwrap(), 30_000);

    diario::definir_bytes_por_volume(0);
    std::fs::remove_dir_all(&d).unwrap();
}

/// O campo aparece na resposta que a tela le, para nao virar campo invisivel.
#[test]
fn o_campo_sai_na_resposta_de_recursos() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let j =
        phxsql_core::json::Json::analisar(r#"{"token":"t","recursos":{"diario_volume_mib":16}}"#)
            .unwrap();
    let c = Config::de_json(&j).unwrap();
    let texto = c.recursos.para_json().escrever();
    assert!(texto.contains("diario_volume_mib"), "{texto}");
    assert!(texto.contains("16"), "{texto}");
}
