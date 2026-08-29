//! A ponta que liga o `config.json` a cifra dos diarios.
//!
//! # Por que um arquivo so para isto
//!
//! `Cifra::aplicar` mexe num global do PROCESSO -- a chave vale para todo
//! diario aberto daqui em diante. Provar isso dentro do binario da biblioteca
//! faria os outros testes do mesmo binario nascerem com a cifra ligada no meio
//! da corrida. Um teste de integracao roda em outro processo, e ali o global e
//! so dele.

use std::path::PathBuf;
use std::sync::Mutex;

use phxsql_core::json::Json;
use phxsql_core::paginacao::Paginacao;
use phxsql_server::config::Config;
use phxsql_store::cofre;
use phxsql_store::log::{LogFile, Operacao};

/// A trava que serializa os testes: o cofre e global ao processo.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn dir(rotulo: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-cfg-cifra-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// O caminho inteiro: `config.json` liga a cifra, e o `.log` nasce cifrado.
///
/// E o teste que responde a pergunta que o projeto ja pagou caro para aprender:
/// o campo de configuracao **e lido por alguma linha de codigo**?
#[test]
fn o_campo_do_config_liga_a_cifra_de_verdade() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("liga");
    let caminho = d.join("config.json");
    std::fs::write(
        &caminho,
        r#"{
          "token": "um token qualquer",
          "base": "dados",
          "cifra": { "ligada": true, "senha": "a chave do cofre", "iteracoes": 10000 }
        }"#,
    )
    .unwrap();

    assert!(!cofre::ligado(), "o processo nao pode comecar com cofre");
    let c = Config::ler(&caminho).unwrap();
    assert!(c.cifra.ligada);
    assert!(
        cofre::ligado(),
        "o config.json ligou a cifra e nenhuma linha de codigo leu o campo"
    );

    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar_com_imagem(Operacao::Inclusao, 1, 1, b"Blumenau")
        .unwrap();
    l.sincronizar().unwrap();
    let bruto = std::fs::read(d.join("t.log")).unwrap();
    assert_eq!(
        u16::from_le_bytes([bruto[8], bruto[9]]),
        3,
        "o .log nasceu na versao velha com a cifra ligada"
    );
    assert!(
        !bruto.windows(8).any(|j| j == b"Blumenau"),
        "o texto claro foi para o disco"
    );

    // E o `para_json` do config -- que a tela le -- nao leva a senha junto.
    let texto = c.para_json().escrever();
    assert!(
        !texto.contains("a chave do cofre"),
        "a senha vazou: {texto}"
    );

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// Cifra ligada sem senha nao sobe -- e o erro diz qual campo preencher.
#[test]
fn cifra_ligada_sem_senha_recusa_a_subir() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("sem-senha");
    let caminho = d.join("config.json");
    std::fs::write(&caminho, r#"{"token":"t","cifra":{"ligada":true}}"#).unwrap();
    let Err(e) = Config::ler(&caminho) else {
        panic!("subiu com a cifra ligada e sem senha")
    };
    assert!(e.to_string().contains("senha"), "{e}");
    assert!(!cofre::ligado());
    std::fs::remove_dir_all(&d).unwrap();
}

/// Config sem a secao `cifra` nao liga nada, e nao vira aviso de campo
/// estranho. E o comportamento velho, que e o que mais importa proteger.
#[test]
fn config_de_ontem_continua_subindo_sem_cifra() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("velho");
    let caminho = d.join("config.json");
    std::fs::write(&caminho, r#"{"token":"t","bind":"0.0.0.0:5000"}"#).unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(!c.cifra.ligada);
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert!(!cofre::ligado(), "um config sem cifra ligou o cofre");

    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar(Operacao::Inclusao, 1, 1).unwrap();
    l.sincronizar().unwrap();
    let bruto = std::fs::read(d.join("t.log")).unwrap();
    assert_eq!(u16::from_le_bytes([bruto[8], bruto[9]]), 2);
    std::fs::remove_dir_all(&d).unwrap();
}

/// O JSON da configuracao inteira nunca carrega a senha da cifra.
#[test]
fn a_resposta_do_protocolo_nao_leva_a_senha() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let j = Json::analisar(
        r#"{"token":"t","cifra":{"ligada":true,"senha":"segredo do cofre","iteracoes":10000}}"#,
    )
    .unwrap();
    let c = Config::de_json(&j).unwrap();
    let texto = c.para_json().escrever();
    assert!(!texto.contains("segredo do cofre"), "{texto}");
    // E nem no `Debug`, que e por onde um diagnostico apressado vazaria.
    assert!(!format!("{:?}", c.cifra).contains("segredo do cofre"));
}
