//! Pedido 495, fatia F2: a camada de ocorrencias pelo servidor de verdade.
//!
//! As provas da camada sozinha moram em `crate::ocorrencias`; aqui mora a
//! que so o servidor faz -- o correio compartilhado com a saude do disco e
//! a thread `sonda-disco` como o carteiro unico.

use super::*;
use crate::apoio_teste::DirTemp;

fn config_base(dir: &std::path::Path) -> Config {
    Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    }
}

/// **O caminho real:** a camada mora no MESMO correio da saude do disco, e
/// o carteiro de verdade leva o alarme marcado na tarefa ao
/// `ocorrencias.log`, ao lado do `acessos.log` -- redigido, com quem e de
/// onde.
///
/// Vermelho: sem o laco do carteiro levar a carta `Ocorrencia` ao
/// `gravar_ocorrencia`, o arquivo fica vazio e o prazo de 10 s vence.
#[test]
fn o_alarme_da_tarefa_chega_redigido_ao_ocorrencias_log_pelo_carteiro() {
    let dir = DirTemp::novo("ocorrencias-f2");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(
        Arc::ptr_eq(s.saude.correio(), s.ocorrencias.correio()),
        "dois correios: dois carteiros"
    );
    s.ligar_sonda_de_disco();
    let sentinela = "SENTINELA-servidor-f2-k7";
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:f2", "dados", "127.0.0.1", 1, agora)
        .expect("a telemetria nasce ligada");
    {
        let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
        a.comecou_pedido("sql", "root", "loja", "", agora);
        crate::telemetria::sinal(
            crate::aquario::Alarme::SenhaEmClaro,
            &format!(r#"{{"op":"sql","texto":"CREATE USER c PASSWORD '{sentinela}'"}}"#),
        );
        a.terminou_pedido("root");
    }
    let caminho = dir.join(crate::ocorrencias::NOME_DO_ARQUIVO);
    let ate = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let texto = loop {
        let t = std::fs::read_to_string(&caminho).unwrap_or_default();
        if t.contains("senha_em_claro") || std::time::Instant::now() > ate {
            break t;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(texto.contains(r#""alarme":"senha_em_claro""#), "{texto:?}");
    assert!(texto.contains(r#""ip":"127.0.0.1""#), "{texto}");
    assert!(texto.contains(r#""tarefa":"dados:f2""#), "{texto}");
    assert!(!texto.contains(sentinela), "a sentinela vazou: {texto}");
}
