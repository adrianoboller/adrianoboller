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
    assert!(
        p.rede_definir_mfa(&ana, 1, true, "").is_err(),
        "so dono/admin"
    );
    p.rede_definir_mfa(&admin, 1, true, "").unwrap();
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
    p.rede_definir_mfa(&admin, 1, false, "").unwrap();
    let conf = std::fs::read_to_string(dados.join("redes/1/servidor.conf")).unwrap();
    assert!(!conf.contains("auth-user-pass-verify"));

    // MEDIO 7: quem tem autenticador prova o codigo para mudar a exigencia.
    let j = p.mfa_iniciar(&admin).unwrap();
    let seg_admin = totp::de_base32(j.texto_ou("segredo", "")).unwrap();
    let cod_a = |t: u64| totp::formatar(totp::totp(&seg_admin, t, 6), 6);
    p.mfa_confirmar(&admin, &cod_a(agora)).unwrap();
    assert!(
        p.rede_definir_mfa(&admin, 1, true, "").is_err(),
        "sem codigo"
    );
    assert!(!p.rede_exige_mfa("1").unwrap());
    p.rede_definir_mfa(&admin, 1, true, &cod_a(agora + 30))
        .unwrap();
    assert!(p.rede_exige_mfa("1").unwrap());

    // M1 da re-revisao: o admin nao zera o PROPRIO autenticador sem codigo
    // -- e, portanto, continua sem conseguir desligar a exigencia sem ele.
    assert!(p.mfa_zerar(&admin, "admin").is_err(), "zerou a si mesmo");
    assert!(p.mfa_ativo(admin.id).unwrap());
    assert!(p.rede_definir_mfa(&admin, 1, false, "").is_err());
    assert!(p.rede_exige_mfa("1").unwrap());

    // MEDIO 4: sem o mfa.chave e com segredo no banco, nada nasce em
    // silencio -- nem no conferir, nem num cadastro novo.
    std::fs::remove_file(dados.join("mfa.chave")).unwrap();
    assert!(p.mfa_conferir(admin.id, &cod_a(agora + 30)).is_err());
    p.criar_usuario("bia", "senha-bia-1", "", false).unwrap();
    let bia = p.login("bia", "senha-bia-1").unwrap();
    assert!(p.mfa_iniciar(&bia).is_err());
    assert!(
        !dados.join("mfa.chave").exists(),
        "recriou a chave em silencio"
    );
    let _ = std::fs::remove_dir_all(&dados);
}

/// Limites 6 e 12 da revisao do MFA: mudar senha, autenticador ou `ativo`
/// derruba as sessoes abertas -- do painel e do token da VPN -- pelo mesmo
/// motor (`credencial.rs`). A sessao de quem mudou a PROPRIA fica.
///
/// RED: com `Sessoes::conferir` ignorando o contador, A2 continua 200 depois
/// do cadastro da ana, e o token da VPN renova depois de zerar.
#[test]
fn sessoes_caem_quando_a_credencial_muda() {
    use phxsql_core::base64::codificar as b64;
    use phxsql_core::json::Json;
    use phxvpn::http::{atender, Estado};
    use phxvpn::totp;
    use phxvpn::web::Pedido;
    let Some(base) = config() else {
        eprintln!("NAO RODOU: defina PHXVPN_PG_TESTE");
        return;
    };
    let cfg = banco_novo(&base, "phxvpn_teste_credencial");
    let dados = std::env::temp_dir().join(format!("phxvpn-teste-cred-{}", std::process::id()));
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
    p.criar_usuario("bia", "senha-bia-1", "", false).unwrap();
    let e = Estado::novo(p, None);

    let pedir = |metodo: &str, caminho: &str, tk: Option<&str>, corpo: &str| -> (u16, Json) {
        let r = atender(
            &Pedido {
                metodo: metodo.into(),
                caminho: caminho.into(),
                token: tk.map(str::to_string),
                host: None,
                tipo: Some("application/json".into()),
                ip: "192.0.2.7".parse().unwrap(),
                corpo: corpo.into(),
            },
            &e,
        );
        (r.status, Json::analisar(&r.corpo).unwrap())
    };
    let entrar = |login: &str, senha: &str| -> String {
        let (s, j) = pedir(
            "POST",
            "/api/login",
            None,
            &format!(r#"{{"usuario":"{login}","senha":"{senha}"}}"#),
        );
        assert_eq!(s, 200, "login {login}: {}", j.escrever());
        j.texto_ou("token", "").to_string()
    };
    let vale = |tk: &str| pedir("GET", "/api/mfa", Some(tk), "").0;

    let ta = entrar("admin", "senha-admin");
    let a1 = entrar("ana", "senha-ana-1");
    let a2 = entrar("ana", "senha-ana-1");

    // A ana liga o PROPRIO autenticador por A1: A1 fica, A2 cai.
    let (s, j) = pedir("POST", "/api/mfa/iniciar", Some(&a1), "{}");
    assert_eq!(s, 200);
    let segredo = totp::de_base32(j.texto_ou("segredo", "")).unwrap();
    let agora = totp::agora();
    let cod = |t: u64| totp::formatar(totp::totp(&segredo, t, 6), 6);
    let (s, _) = pedir(
        "POST",
        "/api/mfa/confirmar",
        Some(&a1),
        &format!(r#"{{"codigo":"{}"}}"#, cod(agora)),
    );
    assert_eq!(s, 200);
    assert_eq!(vale(&a1), 200, "a sessao de quem mudou tinha de ficar");
    assert_eq!(
        vale(&a2),
        401,
        "a outra sessao da ana sobreviveu ao cadastro"
    );

    // Ana entra na rede; a VPN confere senha + codigo (Initial) e depois
    // renova pelo token (Authenticated).
    let (s, _) = pedir(
        "POST",
        "/api/redes/entrar",
        Some(&a1),
        r#"{"nome":"Matriz","senha":"rede-123"}"#,
    );
    assert_eq!(s, 200);
    let mut pg = Pg::conectar(&cfg).unwrap();
    let cn = pg
        .executar(
            "SELECT cn FROM phx_membro WHERE rede_id = 1 AND cn LIKE 'ana.%'",
            &[],
        )
        .unwrap()
        .valor(0, "cn")
        .unwrap()
        .to_string();
    let ccd = dados.join("redes/1/ccd").join(&cn);
    assert!(ccd.exists());
    let vpn = |senha: &str, codigo: &str, estado: &str, sid: &str| {
        let scrv1 = format!("SCRV1:{}:{}", b64(senha.as_bytes()), b64(codigo.as_bytes()));
        let j = phxvpn::verificar::pedido("1", &cn, "ana", &scrv1, "192.0.2.8", (estado, sid));
        phxvpn::verificar::conferir(&e, &j)
    };
    vpn("senha-ana-1", &cod(agora + 30), "Initial", "S1").unwrap();
    vpn("", "", "Authenticated", "S1").unwrap();
    assert!(
        vpn("", "", "Authenticated", "S2").is_err(),
        "sessao que o painel nao abriu"
    );
    assert!(vpn("", "", "Expired", "S1").is_err());

    // O admin zera o autenticador da ana: A1 cai, e o token da VPN nao renova.
    let (s, _) = pedir(
        "POST",
        "/api/usuarios/mfa-zerar",
        Some(&ta),
        r#"{"login":"ana"}"#,
    );
    assert_eq!(s, 200);
    assert_eq!(vale(&a1), 401, "sessao da ana sobreviveu ao zerar");
    let m = vpn("", "", "Authenticated", "S1").unwrap_err();
    assert!(m.contains("a conta mudou"), "{m}");
    assert_eq!(vale(&ta), 200, "a sessao do admin nao e da ana");

    // Desativar: a sessao cai, o login e recusado e o ccd some (a rede que
    // so pede certificado tambem barra). Reativar devolve o ccd.
    let a3 = entrar("ana", "senha-ana-1");
    let ativo = |login: &str, v: bool| {
        pedir(
            "POST",
            "/api/usuarios/ativo",
            Some(&ta),
            &format!(r#"{{"login":"{login}","ativo":{v}}}"#),
        )
        .0
    };
    assert_eq!(ativo("ana", false), 200);
    assert_eq!(vale(&a3), 401, "sessao do desativado sobreviveu");
    assert!(!ccd.exists(), "o ccd do desativado continuou la");
    let (s, _) = pedir(
        "POST",
        "/api/login",
        None,
        r#"{"usuario":"ana","senha":"senha-ana-1"}"#,
    );
    assert_eq!(s, 401);
    assert_eq!(ativo("ana", true), 200);
    assert!(ccd.exists());
    entrar("ana", "senha-ana-1");
    assert_eq!(ativo("admin", false), 400, "o admin desativou a si mesmo");

    // Trocar a propria senha: exige a atual; B1 fica, B2 cai.
    let b1 = entrar("bia", "senha-bia-1");
    let b2 = entrar("bia", "senha-bia-1");
    let trocar = |tk: &str, atual: &str| {
        pedir(
            "POST",
            "/api/senha",
            Some(tk),
            &format!(r#"{{"senha_atual":"{atual}","senha_nova":"senha-bia-2"}}"#),
        )
        .0
    };
    assert_eq!(trocar(&b1, "errada-errada"), 401);
    assert_eq!(vale(&b2), 200, "senha atual errada nao muda nada");
    assert_eq!(trocar(&b1, "senha-bia-1"), 200);
    assert_eq!(vale(&b1), 200);
    assert_eq!(vale(&b2), 401, "a outra sessao da bia sobreviveu a troca");

    // Pelo banco, sem rota nenhuma: o gatilho tambem derruba. E o que nao
    // autentica (o passo do ultimo codigo) nao derruba.
    let b3 = entrar("bia", "senha-bia-2");
    pg.executar(
        "UPDATE phx_usuario SET totp_ultimo = totp_ultimo + 1 WHERE login = 'bia'",
        &[],
    )
    .unwrap();
    assert_eq!(vale(&b3), 200, "totp_ultimo nao e credencial");
    pg.executar(
        "UPDATE phx_usuario SET admin = true WHERE login = 'bia'",
        &[],
    )
    .unwrap();
    assert_eq!(vale(&b3), 401, "UPDATE pelo psql nao derrubou");
    drop(e);
    let _ = std::fs::remove_dir_all(&dados);
}

/// Gerencia do OpenVPN de mentira, no soquete de verdade da rede: responde
/// `status 2` com quem esta em `conectados` (CN, CID) e tira da lista quem
/// recebe `client-kill`. Devolve a lista e os comandos ouvidos.
#[cfg(unix)]
#[allow(clippy::type_complexity)]
fn gerencia_falsa(
    sock: &std::path::Path,
) -> (
    std::sync::Arc<std::sync::Mutex<Vec<(String, u32)>>>,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::{Arc, Mutex};
    let _ = std::fs::remove_file(sock);
    let ouvinte = std::os::unix::net::UnixListener::bind(sock).unwrap();
    let conectados: Arc<Mutex<Vec<(String, u32)>>> = Arc::default();
    let comandos: Arc<Mutex<Vec<String>>> = Arc::default();
    let (c2, k2) = (conectados.clone(), comandos.clone());
    std::thread::spawn(move || {
        for c in ouvinte.incoming().flatten() {
            let mut c = c;
            let _ = c.write_all(b">INFO:OpenVPN Management Interface Version 5\r\n");
            let mut leitor = BufReader::new(c.try_clone().unwrap());
            loop {
                let mut l = String::new();
                if leitor.read_line(&mut l).unwrap_or(0) == 0 {
                    break;
                }
                let l = l.trim().to_string();
                k2.lock().unwrap().push(l.clone());
                let r = if l == "status 2" {
                    let mut r = "HEADER,CLIENT_LIST,Common Name,Real Address,Virtual Address,Virtual IPv6 Address,Bytes Received,Bytes Sent,Connected Since,Connected Since (time_t),Username,Client ID,Peer ID,Data Channel Cipher\r\n".to_string();
                    for (cn, cid) in c2.lock().unwrap().iter() {
                        r += &format!(
                            "CLIENT_LIST,{cn},192.0.2.1:1,10.77.1.9,,1,2,d,1,x,{cid},0,c\r\n"
                        );
                    }
                    r + "END\r\n"
                } else if let Some(cid) = l.strip_prefix("client-kill ") {
                    c2.lock().unwrap().retain(|(_, x)| x.to_string() != cid);
                    "SUCCESS: client-kill command succeeded\r\n".into()
                } else {
                    break;
                };
                let _ = c.write_all(r.as_bytes());
            }
        }
    });
    (conectados, comandos)
}

/// Revisao SEC da revogacao, contra o PG real e uma gerencia de mentira no
/// soquete da rede. Cada bloco nomeia o achado e o RED que o reprova.
#[cfg(unix)]
#[test]
fn revogacao_revisao_sec() {
    use phxsql_core::base64::codificar as b64;
    use phxsql_core::json::Json;
    use phxvpn::credencial::Momento;
    use phxvpn::http::{atender, Estado};
    use phxvpn::totp;
    use phxvpn::web::Pedido;
    let Some(base) = config() else {
        eprintln!("NAO RODOU: defina PHXVPN_PG_TESTE");
        return;
    };
    let cfg = banco_novo(&base, "phxvpn_teste_revisao_sec");
    let dados = std::env::temp_dir().join(format!("phxvpn-teste-sec-{}", std::process::id()));
    let mut p = Painel::abrir(&cfg, &dados).unwrap();
    p.iteracoes = 1_000;
    p.instalar(&Instalacao {
        empresa: "Empresa Teste".into(),
        responsavel: "Fulano".into(),
        email: "f@e.com".into(),
        admin_usuario: "admin".into(),
        admin_senha: "senha-admin-1".into(),
        senha_mestre: "senha-mestre-longa".into(),
        servidor_nome: "vpn1".into(),
        servidor_ip: "203.0.113.10".into(),
        ..Default::default()
    })
    .unwrap();
    let admin = p.login("admin", "senha-admin-1").unwrap();
    p.criar_rede(&admin, "Matriz", "rede-123", "", None)
        .unwrap();
    p.criar_rede(&admin, "Filial", "rede-456", "", None)
        .unwrap();
    for u in ["ana", "bia", "caio"] {
        p.criar_usuario(u, &format!("senha-{u}-1"), "", false)
            .unwrap();
    }
    let e = Estado::novo(p, None);
    let (conectados, comandos) = gerencia_falsa(&dados.join("gerencia/1.sock"));
    e.reconciliar(); // primeira volta: so o retrato

    let pedir = |metodo: &str, caminho: &str, tk: Option<&str>, corpo: &str| -> (u16, Json) {
        let r = atender(
            &Pedido {
                metodo: metodo.into(),
                caminho: caminho.into(),
                token: tk.map(str::to_string),
                host: None,
                tipo: Some("application/json".into()),
                ip: "192.0.2.7".parse().unwrap(),
                corpo: corpo.into(),
            },
            &e,
        );
        (r.status, Json::analisar(&r.corpo).unwrap())
    };
    let entrar = |login: &str| -> String {
        let (s, j) = pedir(
            "POST",
            "/api/login",
            None,
            &format!(r#"{{"usuario":"{login}","senha":"senha-{login}-1"}}"#),
        );
        assert_eq!(s, 200, "login {login}: {}", j.escrever());
        j.texto_ou("token", "").to_string()
    };
    let na_rede = |tk: &str, rede: &str, senha: &str| {
        let (s, j) = pedir(
            "POST",
            "/api/redes/entrar",
            Some(tk),
            &format!(r#"{{"nome":"{rede}","senha":"{senha}"}}"#),
        );
        assert_eq!(s, 200, "{}", j.escrever());
    };
    let cn_de = |login: &str, rede: i64| -> String {
        Pg::conectar(&cfg)
            .unwrap()
            .executar(
                "SELECT m.cn FROM phx_membro m JOIN phx_usuario u ON u.id = m.usuario_id \
             WHERE u.login = $1 AND m.rede_id = $2::int",
                &[Some(login), Some(&rede.to_string())],
            )
            .unwrap()
            .valor(0, "cn")
            .unwrap()
            .to_string()
    };
    let ccd = |rede: i64, cn: &str| dados.join(format!("redes/{rede}/ccd/{cn}"));
    let vale = |tk: &str| pedir("GET", "/api/mfa", Some(tk), "").0;
    let ta = entrar("admin");
    let ativo = |login: &str, v: bool| {
        pedir(
            "POST",
            "/api/usuarios/ativo",
            Some(&ta),
            &format!(r#"{{"login":"{login}","ativo":{v}}}"#),
        )
        .0
    };
    // M3: o admin zera o autenticador da ana ENTRE a gravacao do cadastro
    // e a renovacao da sessao dela. A sessao nao pode adotar a mudanca do
    // admin. RED: renovar relendo o banco (ou sem conferir o +1) deixa A1
    // com 200.
    let a1 = entrar("ana");
    let (s, j) = pedir("POST", "/api/mfa/iniciar", Some(&a1), "{}");
    assert_eq!(s, 200);
    let segredo = totp::de_base32(j.texto_ou("segredo", "")).unwrap();
    let agora = totp::agora();
    let cod = |seg: &[u8], t: u64| totp::formatar(totp::totp(seg, t, 6), 6);
    let adm = admin.clone();
    e.gancho_de_teste(Momento::AntesDeRenovar, move |e| {
        e.painel().mfa_zerar(&adm, "ana").unwrap();
    });
    let (s, _) = pedir(
        "POST",
        "/api/mfa/confirmar",
        Some(&a1),
        &format!(r#"{{"codigo":"{}"}}"#, cod(&segredo, agora)),
    );
    assert_eq!(s, 200);
    assert_eq!(vale(&a1), 401, "a sessao adotou o zerar do admin (M3)");

    // M3, a outra janela: o admin zera o autenticador da ana DEPOIS de
    // conferida a sessao e ANTES de gravar a troca de senha dela. O
    // `RETURNING` devolve +2: a sessao atual tem de cair. RED: renovar
    // aceitando qualquer valor deixa A1b com 200.
    let a1b = entrar("ana");
    let (_, j) = pedir("POST", "/api/mfa/iniciar", Some(&a1b), "{}");
    let seg2 = totp::de_base32(j.texto_ou("segredo", "")).unwrap();
    let (s, _) = pedir(
        "POST",
        "/api/mfa/confirmar",
        Some(&a1b),
        &format!(r#"{{"codigo":"{}"}}"#, cod(&seg2, agora)),
    );
    assert_eq!(s, 200);
    assert_eq!(vale(&a1b), 200);
    let adm = admin.clone();
    e.gancho_de_teste(Momento::AntesDeGravar, move |e| {
        e.painel().mfa_zerar(&adm, "ana").unwrap();
    });
    let (s, j) = pedir(
        "POST",
        "/api/senha",
        Some(&a1b),
        &format!(
            r#"{{"senha_atual":"senha-ana-1","senha_nova":"senha-ana-1","codigo":"{}"}}"#,
            cod(&seg2, agora + 30)
        ),
    );
    assert_eq!(s, 200, "{}", j.escrever());
    assert_eq!(vale(&a1b), 401, "a sessao sobreviveu ao zerar no meio (M3)");

    // M1: a ana em duas redes; o ccd dela na Matriz nao sai (virou pasta
    // com arquivo). Desativar responde ERRO -- e a Filial e processada
    // assim mesmo. Depois de desfeito o obstaculo, a vigia refaz; a volta
    // seguinte nao faz nada. RED: so logar o erro (ou parar na primeira
    // rede) da 200 / deixa o ccd da Filial.
    let a2 = entrar("ana");
    na_rede(&a2, "Matriz", "rede-123");
    na_rede(&a2, "Filial", "rede-456");
    let (cn_ana, cn_ana_2) = (cn_de("ana", 1), cn_de("ana", 2));
    std::fs::remove_file(ccd(1, &cn_ana)).unwrap();
    std::fs::create_dir_all(ccd(1, &cn_ana).join("trava")).unwrap();
    assert_eq!(
        ativo("ana", false),
        500,
        "desativar sem tirar o ccd deu ok (M1)"
    );
    assert!(!ccd(2, &cn_ana_2).exists(), "parou na primeira rede (M1)");
    std::fs::remove_dir_all(ccd(1, &cn_ana)).unwrap();
    let antes = comandos.lock().unwrap().len();
    e.reconciliar();
    assert!(
        comandos.lock().unwrap().len() > antes,
        "a vigia nao refez a mudanca que falhou"
    );
    let depois = comandos.lock().unwrap().len();
    e.reconciliar();
    assert_eq!(
        comandos.lock().unwrap().len(),
        depois,
        "refez o que ja estava feito"
    );

    // M2: `UPDATE` direto no banco, sem rota. A vigia tira o ccd e derruba
    // a conexao. RED: sem a vigia, o ccd fica e ninguem cai.
    let b1 = entrar("bia");
    na_rede(&b1, "Matriz", "rede-123");
    let cn_bia = cn_de("bia", 1);
    conectados.lock().unwrap().push((cn_bia.clone(), 31));
    Pg::conectar(&cfg)
        .unwrap()
        .executar(
            "UPDATE phx_usuario SET ativo = false WHERE login = 'bia'",
            &[],
        )
        .unwrap();
    e.reconciliar();
    assert!(
        !ccd(1, &cn_bia).exists(),
        "UPDATE pelo psql deixou o ccd (M2)"
    );
    assert!(
        comandos
            .lock()
            .unwrap()
            .contains(&"client-kill 31".to_string()),
        "UPDATE pelo psql nao derrubou a conexao (M2)"
    );

    // B1: remover membro derruba a conexao dele, e o token da VPN dele nao
    // renova mais (o vinculo sumiu). RED: sem `derrubar_revogados` no fim
    // do pedido, ninguem cai; sem conferir o vinculo, o token renova.
    let c1 = entrar("caio");
    let (_, j) = pedir("POST", "/api/mfa/iniciar", Some(&c1), "{}");
    let seg_caio = totp::de_base32(j.texto_ou("segredo", "")).unwrap();
    let (s, _) = pedir(
        "POST",
        "/api/mfa/confirmar",
        Some(&c1),
        &format!(r#"{{"codigo":"{}"}}"#, cod(&seg_caio, agora)),
    );
    assert_eq!(s, 200);
    na_rede(&c1, "Matriz", "rede-123");
    let cn_caio = cn_de("caio", 1);
    let vpn = |senha: &str, codigo: &str, estado: &str| {
        let scrv1 = format!("SCRV1:{}:{}", b64(senha.as_bytes()), b64(codigo.as_bytes()));
        let j =
            phxvpn::verificar::pedido("1", &cn_caio, "caio", &scrv1, "192.0.2.8", (estado, "S7"));
        phxvpn::verificar::conferir(&e, &j)
    };
    vpn("senha-caio-1", &cod(&seg_caio, agora + 30), "Initial").unwrap();
    vpn("", "", "Authenticated").unwrap();
    conectados.lock().unwrap().push((cn_caio.clone(), 44));
    let (s, _) = pedir(
        "POST",
        "/api/redes/remover",
        Some(&ta),
        r#"{"rede_id":1,"login":"caio"}"#,
    );
    assert_eq!(s, 200);
    assert!(
        comandos
            .lock()
            .unwrap()
            .contains(&"client-kill 44".to_string()),
        "remover membro nao derrubou a conexao (B1)"
    );
    let m = vpn("", "", "Authenticated").unwrap_err();
    assert!(m.contains("membro"), "{m}");
    drop(e);
    let _ = std::fs::remove_dir_all(&dados);
}
