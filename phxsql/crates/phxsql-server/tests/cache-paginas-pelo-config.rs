//! A ponta que liga o `config.json` ao teto do cache de paginas do `.ndx`.
//!
//! Arquivo proprio pela mesma razao do da cifra, do corte do diario e da
//! exclusao na janela: `phxsql_store::ndx::definir_cache_paginas` mexe num
//! global do PROCESSO, e ligar isto dentro do binario da biblioteca faria o
//! teto de um teste virar o de outro na mesma corrida.
//!
//! # Por que este arquivo existe -- e por que so agora
//!
//! `recursos.cache_paginas` e o campo que deu nome a armadilha "configuracao
//! que nao e lida mente": ele passou tres versoes no `config.json`, no MANUAL
//! e na tela sem que uma linha de codigo o lesse. Isso ja foi corrigido --
//! `Servidor::novo` chama `ndx::definir_cache_paginas` desde a 0.14.0 -- mas a
//! CORRECAO nunca ganhou o teste ponta-a-ponta que os dois campos irmaos
//! ganharam (`exclusao-na-janela-pelo-config.rs`,
//! `corte-do-diario-pelo-config.rs`, os dois citando esta mesma armadilha no
//! comentario). Achado do QA-PDCA: o unico teste que tocava o campo
//! (`campo_dentro_de_secao_muda_so_ele`, em `config.rs`) confere que o
//! `Config` em memoria guarda o valor -- nao que o valor chega ao motor.
//!
//! # Por que `Servidor::novo`, e nao so `Config::ler`
//!
//! Os dois irmaos bastam com `Config::ler`, porque o campo deles e aplicado
//! DENTRO de `Recursos::aplicar` -- e `Config::ler` chama `aplicar` sozinho.
//! `cache_paginas` e diferente por decisao propria (comentario de
//! `Recursos::aplicar`, em `config.rs`): "o teto do cache de paginas continua
//! sendo aplicado pelo servidor, onde ja estava". Testar so `Config::ler`
//! aqui provaria a leitura do JSON e nao provaria nada do motor -- por isso
//! este teste sobe um `Servidor` de verdade.

use std::path::PathBuf;
use std::sync::Mutex;

use phxsql_server::config::{Config, Recursos};
use phxsql_server::servidor::Servidor;
use phxsql_store::ndx;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn dir(rotulo: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-cfg-cache-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn escrever_config(d: &std::path::Path, conteudo: &str) -> PathBuf {
    let caminho = d.join("config.json");
    std::fs::write(&caminho, conteudo).unwrap();
    caminho
}

fn servidor_de(caminho: &std::path::Path) -> std::sync::Arc<Servidor> {
    let c = Config::ler(caminho).unwrap();
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    Servidor::novo(c).unwrap()
}

/// **O teste do comportamento VELHO.** Um `config.json` sem o campo sobe o
/// servidor com o teto PADRAO -- e nao com o que sobrou do teste anterior
/// nesta mesma bateria, que e por isso que o global volta ao padrao antes de
/// cada teste, e nao so quando o campo aparece.
#[test]
fn config_sem_o_campo_sobe_com_o_teto_padrao() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let padrao = Recursos::default().cache_paginas;
    ndx::definir_cache_paginas(padrao);
    let d = dir("padrao");
    let caminho = escrever_config(&d, r#"{"token":"t","recursos":{"threads":2}}"#);

    let _s = servidor_de(&caminho);
    assert_eq!(
        ndx::cache_paginas(),
        padrao,
        "um config.json sem o campo mudou o teto do cache"
    );
}

/// O campo do config CHEGA ao motor -- e e o proprio `ndx::cache_paginas()`
/// que a telemetria devolve em `cache_ndx.paginas_teto`
/// (`servidor.rs`, `op_telemetria`), entao provar o global aqui prova o que a
/// tela mostra.
///
/// Prova real: comente a chamada `phxsql_store::ndx::definir_cache_paginas`
/// em `Servidor::novo` e este teste cai -- o campo volta a ficar so no
/// arquivo, no MANUAL e na tela, prometendo um cache que ninguem ajusta.
#[test]
fn o_campo_do_config_chega_ao_motor_no_arranque() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    ndx::definir_cache_paginas(Recursos::default().cache_paginas);
    let d = dir("chega");
    let caminho = escrever_config(&d, r#"{"token":"t","recursos":{"cache_paginas":128}}"#);

    let c = Config::ler(&caminho).unwrap();
    assert_eq!(c.recursos.cache_paginas, 128);
    let _s = servidor_de(&caminho);
    assert_eq!(
        ndx::cache_paginas(),
        128,
        "o config.json pediu 128 paginas e o motor nao usou esse teto"
    );
}

/// E o caminho de volta: um segundo `Servidor::novo`, com um teto diferente,
/// muda o global outra vez -- o teto nao fica preso no primeiro valor que
/// algum config.json pediu.
#[test]
fn um_segundo_arranque_com_outro_valor_muda_o_teto_de_novo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    ndx::definir_cache_paginas(Recursos::default().cache_paginas);

    let c1 = escrever_config(
        &dir("segundo-um"),
        r#"{"token":"t","recursos":{"cache_paginas":64}}"#,
    );
    let _s1 = servidor_de(&c1);
    assert_eq!(ndx::cache_paginas(), 64);

    let c2 = escrever_config(
        &dir("segundo-dois"),
        r#"{"token":"t","recursos":{"cache_paginas":4096}}"#,
    );
    let _s2 = servidor_de(&c2);
    assert_eq!(
        ndx::cache_paginas(),
        4096,
        "o segundo arranque nao substituiu o teto do primeiro"
    );
}
