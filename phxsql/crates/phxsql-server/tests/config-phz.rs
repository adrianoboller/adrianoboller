//! O `.phz` dos JSON de configuracao (pedido 450, etapa 2), pelo caminho que
//! o servidor usa de verdade: `Config::ler`, a gravacao da tela
//! (`Config::gravar_campos`), a lista de bloqueio e a conversao pedida.

mod comum;

use std::path::Path;

use phxsql_core::json::Json;
use phxsql_server::config::{avisos_do_phz, converter_configs};
use phxsql_server::{Blacklist, Config, Politica};

use comum::DirTemp;

const CONFIG: &str = r#"{
  "token": "token-de-teste",
  "base": "dados",
  "max_linhas": 500,
  "seguranca": {"blacklist": "blacklist.json"},
  "dblink": "dblink.json",
  "jobs": "jobs.json"
}"#;

fn montar(d: &Path) {
    std::fs::write(d.join("config.json"), CONFIG).unwrap();
    std::fs::write(d.join("dblink.json"), r#"{"ligacoes":[]}"#).unwrap();
}

#[test]
fn a_conversao_pedida_fecha_os_arquivos_e_o_servidor_le_pelo_nome_de_sempre() {
    let d = DirTemp::novo("phz-conv");
    montar(&d.0);
    let c = Config::ler(d.0.join("config.json")).unwrap();
    // Em claro, o arranque DIZ, com o comando que converte.
    assert!(avisos_do_phz(&c)
        .iter()
        .any(|a| a.contains("--zipar-config")));

    let relato = converter_configs(&c, true);
    assert!(
        relato
            .iter()
            .any(|l| l.contains("config.phz") && l.contains("conferido")),
        "{relato:?}"
    );
    assert!(
        relato
            .iter()
            .any(|l| l.contains("dblink.phz") && l.contains("conferido")),
        "{relato:?}"
    );
    // O que nao existia continua nao existindo, e e dito.
    assert!(
        relato
            .iter()
            .any(|l| l.contains("jobs.json") && l.contains("nao existe")),
        "{relato:?}"
    );
    assert!(
        !d.0.join("config.json").exists(),
        "o claro tinha de sair depois de conferido"
    );
    assert!(!d.0.join("dblink.json").exists());
    // Em claro nao se le mais: nem o token, nem o nome do campo.
    let bytes = std::fs::read(d.0.join("config.phz")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("token"));

    // Pedido pelo nome de sempre, sobe do .phz, sem aviso de claro.
    let c = Config::ler(d.0.join("config.json")).unwrap();
    assert_eq!(c.max_linhas, 500);
    assert!(avisos_do_phz(&c).is_empty(), "{:?}", avisos_do_phz(&c));
}

#[test]
fn a_tela_grava_no_phz_e_o_irmao_novo_nasce_phz() {
    let d = DirTemp::novo("phz-tela");
    montar(&d.0);
    let c = Config::ler(d.0.join("config.json")).unwrap();
    converter_configs(&c, true);
    let c = Config::ler(d.0.join("config.json")).unwrap();

    // A gravacao da tela de Configuracoes: continua .phz, nunca reabre o claro.
    let novo = Config::gravar_campos(
        &d.0.join("config.json"),
        &[("max_linhas".to_string(), Json::de_u64(777))],
    )
    .unwrap();
    assert_eq!(novo.max_linhas, 777);
    assert!(
        !d.0.join("config.json").exists(),
        "a tela reabriu o config em claro"
    );
    assert!(phxsql_core::phz::ler_texto(&d.0.join("config.phz"))
        .unwrap()
        .contains("777"));

    // A lista de bloqueio nao existia: nasce .phz, porque o config e .phz.
    let mut bl = Blacklist::abrir(&c.blacklist).unwrap();
    bl.bloquear(
        "203.0.113.9",
        "teste",
        "entrar",
        5,
        &Politica::default(),
        1_790_000_000_000,
    );
    assert!(
        d.0.join("blacklist.phz").exists(),
        "o irmao novo nasceu em claro"
    );
    assert!(!d.0.join("blacklist.json").exists());
    let de_novo = Blacklist::abrir(&c.blacklist).unwrap();
    assert!(
        de_novo
            .bloqueado("203.0.113.9", 1_790_000_000_001)
            .is_some(),
        "o bloqueio nao voltou do .phz"
    );
}

#[test]
fn com_os_dois_no_disco_a_conversao_recusa_e_o_arranque_avisa() {
    let d = DirTemp::novo("phz-dois");
    montar(&d.0);
    let c = Config::ler(d.0.join("config.json")).unwrap();
    converter_configs(&c, true);
    // Alguem restaurou um config.json velho ao lado do .phz.
    std::fs::write(d.0.join("config.json"), CONFIG.replace("500", "1")).unwrap();
    let c = Config::ler(d.0.join("config.json")).unwrap();
    assert_eq!(c.max_linhas, 500, "vale o .phz, e nao o claro que sobrou");
    assert!(avisos_do_phz(&c)
        .iter()
        .any(|a| a.contains("sobrou em claro")));
    let relato = converter_configs(&c, true);
    assert!(
        relato
            .iter()
            .any(|l| l.contains("config.phz") && l.contains("ja convertido")
                || l.contains("RECUSADO")),
        "{relato:?}"
    );
    assert!(
        d.0.join("config.json").exists(),
        "a conversao apagou um arquivo que nao converteu"
    );
}

#[test]
fn a_volta_devolve_o_json_identico() {
    let d = DirTemp::novo("phz-volta");
    montar(&d.0);
    let c = Config::ler(d.0.join("config.json")).unwrap();
    converter_configs(&c, true);
    let c = Config::ler(d.0.join("config.json")).unwrap();
    let relato = converter_configs(&c, false);
    assert!(relato.iter().all(|l| !l.contains("FALHOU")), "{relato:?}");
    assert_eq!(
        std::fs::read_to_string(d.0.join("config.json")).unwrap(),
        CONFIG
    );
    assert!(!d.0.join("config.phz").exists());
}

/// O `.phz` e 7z de verdade: o 7-Zip abre com a senha fixa e ve `config.json`.
/// Sem o `7z` na maquina a prova nao roda -- e diz isso.
#[test]
fn o_7zip_abre_o_config_phz() {
    if std::process::Command::new("7z")
        .arg("i")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("NAO MEDIDO: 7z ausente; interoperabilidade do .phz nao conferida");
        return;
    }
    let d = DirTemp::novo("phz-7z");
    montar(&d.0);
    let c = Config::ler(d.0.join("config.json")).unwrap();
    converter_configs(&c, true);
    let o = std::process::Command::new("7z")
        .arg("x")
        .arg(format!("-p{}", phxsql_core::phz::SENHA_FIXA))
        .arg(format!("-o{}", d.0.join("x").display()))
        .arg(d.0.join("config.phz"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(
        std::fs::read_to_string(d.0.join("x/config.json")).unwrap(),
        CONFIG
    );
}
