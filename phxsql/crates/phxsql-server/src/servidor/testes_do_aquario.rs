//! O esqueleto do aquario (pedido 707, fatia A2): quem pode perguntar, e o
//! portao da telemetria antes do `anotar`.
//!
//! O que estes testes travam:
//!
//! 1. **op sem direito declarado cai** -- as duas ops pedem `monitorar`, e
//!    quem tem so `monitorar` passa; sem a linha no `da_operacao` elas cairiam
//!    no `_ => Administrar` e este usuario seria barrado em silencio;
//! 2. **ver nao e matar** -- `monitorar` nao alcanca `telemetria_encerrar` nem
//!    `encerrar_sessao`;
//! 3. **o campo que o portao le e o furo** -- `monitorar` numa base so nao
//!    vira o servidor inteiro mandando `"database"`;
//! 4. **comportamento velho** -- quem administra continua vendo, e sem
//!    cadastro nada muda;
//! 5. **telemetria desligada, aquario nao anota nada**.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("aq-{nome}"))
}

fn cadastro(nivel: &str, bases: &str) -> Cadastro {
    Cadastro::de_json(
        &Json::analisar(&format!(
            r#"{{"usuarios":[{{"login":"ana","id":9,"nivel":"{nivel}",
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
        ))
        .unwrap(),
    )
    .unwrap()
}

fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro,
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade -- os dois
/// portoes, o geral e o de dentro, no caminho.
fn pede(s: &Arc<Servidor>, c: &Cadastro, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        usuario: c.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

const AS_DUAS: [&str; 2] = [r#""op":"aquario_log""#, r#""op":"aquario_contagens""#];

/// Passou do portao: o que volta e o «ainda nao existe» da fatia que falta,
/// e nao o `ACESSO_NEGADO`. Conferir o NOME do erro e nao so `is_err` e o que
/// impede o teste de passar por engano.
fn passou_do_portao(r: Result<Json>, corpo: &str) {
    let e = r.expect_err("A6 e A8 ainda nao entraram: a op nao pode fingir que respondeu");
    assert_eq!(e.nome(), "NAO_ENCONTRADO", "{corpo}: {e}");
    assert!(e.to_string().contains("ainda nao"), "{corpo}: {e}");
}

fn barrado(r: Result<Json>, corpo: &str) {
    let e = r.expect_err(corpo);
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
}

/// O RED da A2. Leitor com `monitorar` no curinga, e nada mais de servidor:
/// passa nas duas. Tirar a linha `"aquario_log" | "aquario_contagens"` do
/// `da_operacao` as joga no `_ => Administrar`, e este teste cai.
#[test]
fn quem_tem_monitorar_passa_nas_duas() {
    let dir = dir_temp("monitor");
    let c = cadastro("leitor", r#"{"*":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in AS_DUAS {
        passou_do_portao(pede(&s, &c, corpo), corpo);
    }
    assert_eq!(
        Atividade::da_operacao("aquario_log"),
        Some(Atividade::Monitorar)
    );
    assert_eq!(
        Atividade::da_operacao("aquario_contagens"),
        Some(Atividade::Monitorar)
    );
}

#[test]
fn leitor_sem_monitorar_nao_ve_o_aquario() {
    let dir = dir_temp("leitor");
    let c = cadastro("leitor", r#"{"*":{"ler":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in AS_DUAS {
        barrado(pede(&s, &c, corpo), corpo);
    }
}

/// Ver nao e matar: a convergencia dos tres maduros que fez o direito nascer.
#[test]
fn monitorar_nao_mata() {
    let dir = dir_temp("nao-mata");
    let c = cadastro("leitor", r#"{"*":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in [
        r#""op":"telemetria_encerrar","id":"dados:1""#,
        r#""op":"encerrar_sessao","id":1,"tipo":"conexao""#,
    ] {
        barrado(pede(&s, &c, corpo), corpo);
    }
}

/// `monitorar` so na base `loja`: o portao geral, que le `"database"`,
/// deixaria passar o pedido que nomeia `loja`. O de dentro pergunta na regra
/// do servidor e recusa, com o motivo.
#[test]
fn monitorar_numa_base_so_nao_ve_o_servidor_inteiro() {
    let dir = dir_temp("uma-base");
    let c = cadastro("leitor", r#"{"loja":{"ler":true,"monitorar":true}}"#);
    let s = servidor(&dir, c.clone());
    for corpo in [
        r#""op":"aquario_log","database":"loja""#,
        r#""op":"aquario_contagens","database":"loja""#,
        r#""op":"aquario_log""#,
    ] {
        let e = pede(&s, &c, corpo).expect_err(corpo);
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
    }
    let e = pede(&s, &c, r#""op":"aquario_log","database":"loja""#).unwrap_err();
    assert!(e.to_string().contains("monitorar este servidor"), "{e}");
}

/// O comportamento VELHO: quem administrava ja via tudo, e continua vendo --
/// pelo nivel e pela regra curinga, sem nenhum `"monitorar": true` escrito.
#[test]
fn quem_administra_continua_vendo() {
    for (nivel, bases) in [
        ("admin", r#"{}"#),
        ("leitor", r#"{"*":{"ler":true,"administrar":true}}"#),
    ] {
        let dir = dir_temp(&format!("adm-{nivel}"));
        let c = cadastro(nivel, bases);
        let s = servidor(&dir, c.clone());
        for corpo in AS_DUAS {
            passou_do_portao(pede(&s, &c, corpo), corpo);
        }
    }
}

#[test]
fn sem_cadastro_nada_muda() {
    let dir = dir_temp("sem-cadastro");
    let s = servidor(&dir, Cadastro::default());
    for corpo in AS_DUAS {
        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        passou_do_portao(r, corpo);
    }
}

/// Instrumentacao desligada custa zero, e o portao vem ANTES do trabalho:
/// com a telemetria desligada o aquario nao ve pedido nenhum. Tirar o
/// `then_some` do `aquario_se_ligada` (devolver sempre o aquario) faz este
/// teste cair.
#[test]
fn telemetria_desligada_o_aquario_nao_anota() {
    let dir = dir_temp("desligada");
    let s = servidor(&dir, Cadastro::default());
    let acesso = Acesso {
        op: "ping".into(),
        ok: true,
        ..Acesso::default()
    };
    s.telemetria.desligar();
    let antes = s.telemetria.aquario().anotados();
    for _ in 0..5 {
        s.anotar(&acesso);
    }
    assert_eq!(
        s.telemetria.aquario().anotados(),
        antes,
        "desligada e anotou"
    );

    s.telemetria.ligar(crate::agora_ms());
    s.anotar(&acesso);
    assert_eq!(
        s.telemetria.aquario().anotados(),
        antes + 1,
        "ligada e nao anotou: o gancho do anotar sumiu"
    );
}
