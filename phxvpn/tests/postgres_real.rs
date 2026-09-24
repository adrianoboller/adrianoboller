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
    let perfil_velho = p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();
    // Reentrar mantem o IP (.3), troca o certificado e REVOGA o anterior:
    // o perfil do notebook roubado nao volta a valer (achado A1).
    let perfil_novo = p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();
    let membros = p.membros(&ana, 1).unwrap().escrever();
    assert!(membros.contains("10.77.1.3"), "{membros}");
    let ccd_da_ana = || -> Vec<String> {
        std::fs::read_dir(dados.join("redes/1/ccd"))
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.starts_with("ana.1."))
            .collect()
    };
    assert_eq!(
        ccd_da_ana().len(),
        1,
        "o ccd do perfil velho tinha de sumir"
    );
    conferir_na_crl(&dados, &perfil_velho, &perfil_novo);

    // Painel reaberto nasce trancado; senha mestre errada nao destranca.
    drop(p);
    let mut p = Painel::abrir(&cfg, &dados).unwrap();
    assert!(p.entrar_na_rede(&ana, "Matriz", "rede-123").is_err());
    assert!(p.destrancar("senha-mestre-errada").is_err());
    p.destrancar("senha-mestre-longa").unwrap();
    p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();

    p.sair_da_rede(&ana, 1).unwrap();
    assert!(ccd_da_ana().is_empty());
    let _ = std::fs::remove_dir_all(&dados);
}

/// Com o `openssl` do sistema: o certificado do perfil velho esta revogado na
/// CRL que o painel gravou para o OpenVPN; o do perfil novo passa.
fn conferir_na_crl(dados: &std::path::Path, velho: &str, novo: &str) {
    let Some(openssl) = phxvpn::supervisor::achar_no_path("openssl") else {
        eprintln!("NAO RODOU a prova da CRL: sem openssl no PATH");
        return;
    };
    let cert = |perfil: &str| {
        let a = perfil.find("<cert>\n").unwrap() + 7;
        let b = perfil.find("</cert>").unwrap();
        perfil[a..b].to_string()
    };
    let d = dados.join("redes/1");
    std::fs::write(d.join("velho.pem"), cert(velho)).unwrap();
    std::fs::write(d.join("novo.pem"), cert(novo)).unwrap();
    let verificar = |c: &str| {
        std::process::Command::new(&openssl)
            .current_dir(&d)
            .args([
                "verify",
                "-crl_check",
                "-CAfile",
                "ca.crt",
                "-CRLfile",
                "crl.pem",
                c,
            ])
            .output()
            .unwrap()
    };
    let v = verificar("velho.pem");
    let texto =
        String::from_utf8_lossy(&v.stderr).to_string() + &String::from_utf8_lossy(&v.stdout);
    assert!(
        !v.status.success() && texto.contains("revoked"),
        "perfil velho nao revogado: {texto}"
    );
    assert!(verificar("novo.pem").status.success(), "perfil novo caiu");
}

/// O segundo fator no banco: cadastro em duas etapas, conferencia sem
/// reuso, rede que exige, e o segredo que nao volta em resposta nenhuma.
#[test]
fn autenticador_cadastro_reuso_e_rede_que_exige() {
    use phxvpn::totp;
    let Some(base) = config() else {
        eprintln!("NAO RODOU: defina PHXVPN_PG_TESTE");
        return;
    };
    let cfg = banco_novo(&base, "phxvpn_teste_mfa");
    let dados = std::env::temp_dir().join(format!("phxvpn-teste-mfa-{}", std::process::id()));
    let mut p = Painel::abrir(&cfg, &dados).unwrap();
    p.iteracoes = 1_000;
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
    p.criar_rede(&admin, "Matriz", "rede-123", "", None)
        .unwrap();
    p.criar_usuario("ana", "senha-ana-1", "", false).unwrap();
    let ana = p.login("ana", "senha-ana-1").unwrap();

    // Rede exigindo: quem nao cadastrou e recusado no «entrar», com o motivo.
    assert!(p.rede_definir_mfa(&ana, 1, true).is_err(), "so dono/admin");
    p.rede_definir_mfa(&admin, 1, true).unwrap();
    let e = p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap_err();
    assert!(e.contains("exige o autenticador"), "{e}");
    let conf = std::fs::read_to_string(dados.join("redes/1/servidor.conf")).unwrap();
    assert!(conf.contains("auth-user-pass-verify") && conf.contains("via-file"));

    // Cadastro: iniciar da o segredo UMA vez; codigo errado nao ativa.
    assert!(!p.mfa_ativo(ana.id).unwrap());
    let j = p.mfa_iniciar(&ana).unwrap();
    let segredo = totp::de_base32(j.texto_ou("segredo", "")).unwrap();
    assert_eq!(segredo.len(), totp::SEGREDO_LEN);
    assert!(j.texto_ou("uri", "").starts_with("otpauth://totp/"));
    let agora = totp::agora();
    let cod = |t: u64| totp::formatar(totp::totp(&segredo, t, 6), 6);
    let errado = if cod(agora) == "000000" {
        "111111"
    } else {
        "000000"
    };
    assert!(p.mfa_confirmar(&ana, errado).is_err());
    assert!(!p.mfa_ativo(ana.id).unwrap());
    p.mfa_confirmar(&ana, &cod(agora)).unwrap();
    assert!(p.mfa_ativo(ana.id).unwrap());
    assert!(p.mfa_iniciar(&ana).is_err(), "trocar exige desativar antes");

    // O codigo do cadastro nao vale de novo; o do passo seguinte vale uma vez.
    assert!(p.mfa_conferir(ana.id, &cod(agora)).is_err(), "reuso");
    p.mfa_conferir(ana.id, &cod(agora + 30)).unwrap();
    assert!(p.mfa_conferir(ana.id, &cod(agora + 30)).is_err(), "reuso");

    // O segredo nao esta em claro no banco nem na lista de usuarios.
    let b32 = totp::base32_sem_preenchimento(&segredo);
    let usuarios = p.usuarios().unwrap().escrever();
    assert!(!usuarios.contains(&b32) && usuarios.contains("\"mfa\":true"));
    let mut pg = Pg::conectar(&cfg).unwrap();
    let r = pg
        .executar(
            "SELECT totp_selado FROM phx_usuario WHERE login = 'ana'",
            &[],
        )
        .unwrap();
    let selado = r.valor(0, "totp_selado").unwrap();
    assert!(!selado.contains(&b32));
    assert!(!selado.contains(&phxsql_core::hash::para_hex(&segredo)));

    // Agora a ana entra, e o perfil pede usuario, senha e codigo.
    let perfil = p.entrar_na_rede(&ana, "Matriz", "rede-123").unwrap();
    assert!(perfil.contains("auth-user-pass\n"));
    assert!(perfil.contains("static-challenge \"Código do autenticador\" 1\n"));

    // Admin zera; o dono dispensa e o conf perde o verificador.
    p.mfa_zerar(&admin, "ana").unwrap();
    assert!(!p.mfa_ativo(ana.id).unwrap());
    p.rede_definir_mfa(&admin, 1, false).unwrap();
    let conf = std::fs::read_to_string(dados.join("redes/1/servidor.conf")).unwrap();
    assert!(!conf.contains("auth-user-pass-verify"));
    let _ = std::fs::remove_dir_all(&dados);
}
