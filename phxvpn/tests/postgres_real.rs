//! Prova contra um PostgreSQL DE VERDADE: o protocolo de fio, o SCRAM e as
//! regras do painel so se provam contra o servidor, nao contra um duble.
//!
//! Roda quando `PHXVPN_PG_TESTE` aponta para um banco descartavel, por exemplo
//! `host=127.0.0.1 port=55432 user=postgres password=... dbname=postgres`.
//! Sem a variavel o teste diz que NAO rodou -- nunca passa calado.

use phxvpn::painel::{Instalacao, Painel};
use phxvpn::pg::{Config, Pg};

fn config() -> Option<Config> {
    let texto = std::env::var("PHXVPN_PG_TESTE").ok()?;
    Some(Config::de_texto(&texto).expect("PHXVPN_PG_TESTE malformada"))
}

/// Cria um banco novo so para esta corrida e devolve a config apontando nele.
fn banco_novo(base: &Config, nome: &str) -> Config {
    let mut pg = Pg::conectar(base).expect("conectar no PostgreSQL de teste");
    let _ = pg.lote(&format!("DROP DATABASE IF EXISTS {nome}"));
    pg.lote(&format!("CREATE DATABASE {nome}")).unwrap();
    Config {
        banco: nome.into(),
        ..base.clone()
    }
}

#[test]
fn senha_errada_do_postgres_e_recusada() {
    let Some(mut cfg) = config() else {
        eprintln!("NAO RODOU: defina PHXVPN_PG_TESTE");
        return;
    };
    cfg.senha.push_str("-errada");
    let e = Pg::conectar(&cfg)
        .err()
        .expect("senha errada tinha de falhar");
    assert!(
        e.contains("28P01"),
        "esperava falha de autenticacao, veio: {e}"
    );
}

#[test]
fn parametro_nao_vira_sql() {
    let Some(cfg) = config() else {
        eprintln!("NAO RODOU: defina PHXVPN_PG_TESTE");
        return;
    };
    let mut pg = Pg::conectar(&cfg).unwrap();
    let r = pg
        .executar(
            "SELECT $1::text AS v, $2::text IS NULL AS nulo",
            &[Some("'; DROP TABLE x; --"), None],
        )
        .unwrap();
    assert_eq!(r.valor(0, "v"), Some("'; DROP TABLE x; --"));
    assert_eq!(r.valor(0, "nulo"), Some("t"));
}

#[test]
fn ciclo_completo_instalar_criar_entrar_reiniciar() {
    let Some(base) = config() else {
        eprintln!("NAO RODOU: defina PHXVPN_PG_TESTE");
        return;
    };
    let cfg = banco_novo(&base, "phxvpn_teste_ciclo");
    let dados = std::env::temp_dir().join(format!("phxvpn-teste-{}", std::process::id()));
    let mut p = Painel::abrir(&cfg, &dados).unwrap();
    p.iteracoes = 1_000;
    assert!(!p.instalado().unwrap());
    p.instalar(&Instalacao {
        empresa: "Empresa Teste".into(),
        responsavel: "Fulano".into(),
        email: "f@e.com".into(),
        admin_usuario: "admin".into(),
        admin_senha: "senha-admin".into(),
        senha_mestre: "senha-mestre-longa".into(),
        servidor_nome: "vpn1".into(),
        servidor_ip: "203.0.113.10".into(),
        ..Default::default()
    })
    .unwrap();
    let admin = p.login("admin", "senha-admin").unwrap();
    assert!(admin.admin);
    assert!(p.login("admin", "outra").is_err());

    let perfil = p
        .criar_rede(&admin, "Matriz", "rede-123", "ERP", None)
        .unwrap();
    assert!(perfil.contains("remote 203.0.113.10 1195"));
    assert!(p
        .criar_rede(&admin, "Matriz", "rede-123", "", None)
        .is_err());

    p.criar_usuario("ana", "senha-ana-1", "", false).unwrap();
    let ana = p.login("ana", "senha-ana-1").unwrap();
    assert!(p.entrar_na_rede(&ana, "Matriz", "errada").is_err());
    p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();
    // Reentrar mantem o IP (.3) e troca o certificado.
    p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();
    let membros = p.membros(&ana, 1).unwrap().escrever();
    assert!(membros.contains("10.77.1.3"), "{membros}");
    assert!(dados.join("redes/1/ccd/ana.1").is_file());

    // Painel reaberto nasce trancado; senha mestre errada nao destranca.
    drop(p);
    let mut p = Painel::abrir(&cfg, &dados).unwrap();
    assert!(p.entrar_na_rede(&ana, "Matriz", "rede-123").is_err());
    assert!(p.destrancar("senha-mestre-errada").is_err());
    p.destrancar("senha-mestre-longa").unwrap();
    p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();

    p.sair_da_rede(&ana, 1).unwrap();
    assert!(!dados.join("redes/1/ccd/ana.1").exists());
    let _ = std::fs::remove_dir_all(&dados);
}
