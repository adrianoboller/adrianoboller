use super::*;
use crate::apoio_teste::DirTemp;

fn config_com_caminho(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.join("dados"),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "token-que-todo-cliente-tem".into(),
        ..Config::default()
    };
    c.caminho = Some(dir.join("config.json"));
    c
}

fn sal_de(s: &Servidor, login: &str) -> String {
    let mut ses = Sessao::default();
    let r = s
        .op_desafio(
            &Json::analisar(&format!(r#"{{"usuario":"{login}"}}"#)).unwrap(),
            &mut ses,
        )
        .unwrap();
    r.texto_ou("sal", "").to_string()
}

/// **Pedido 528:** o sal de quem NAO existe era `HMAC(token, login)`, e
/// quem tem o token -- todo cliente -- o recalculava e sabia quem nao
/// existe num pedido so. Agora a chave e um segredo do servidor, gravado
/// 0600 ao lado do config na primeira subida: a sonda erra, e o sal falso
/// continua ESTAVEL entre reinicios (senao perguntar antes e depois
/// separaria quem existe).
///
/// # Prova real
///
/// Com o `hmac_sha256` do `op_desafio` voltando a usar o token, a sonda
/// acerta -- o vermelho medido.
#[test]
fn a_sonda_do_token_nao_acerta_o_sal_falso_e_ele_e_estavel() {
    let dir = DirTemp::novo("528-sal-falso");
    let c = config_com_caminho(&dir);
    let s = Servidor::novo(c.clone()).unwrap();
    let sal = sal_de(&s, "fantasma");
    let sonda = phxsql_core::hash::para_hex(
        &phxsql_core::hash::hmac_sha256(c.token.as_bytes(), b"fantasma")[..16],
    );
    assert_eq!(sal.len(), 32, "{sal}");
    assert_ne!(sal, sonda, "quem tem o token recalculou o sal falso");
    drop(s);

    let arquivo = dir.join("segredo-do-desafio.hex");
    assert!(
        arquivo.exists(),
        "o segredo nao foi gravado ao lado do config"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let modo = std::fs::metadata(&arquivo).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600, "o segredo nasceu {modo:o}");
    }
    let s = Servidor::novo(c).unwrap();
    assert_eq!(sal_de(&s, "fantasma"), sal, "o sal falso mudou no reinicio");
}
