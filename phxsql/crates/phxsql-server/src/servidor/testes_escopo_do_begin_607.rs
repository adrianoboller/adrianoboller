//! Pedido 607: o `begin` com `SCOPE` toma trava de tabela NA ABERTURA, e ia ao
//! ar so com o token -- sem login, sem direito na tabela e com o prazo que o
//! cliente escolhesse (`timeout_ms` de 10^12 = 31 anos).
//!
//! # Prova real
//!
//! Com o defeito reposto (o `pede_identidade` devolvendo so o
//! `da_operacao`, o `declarar_escopo` sem o laco do `pode_em`, o
//! `.min(teto_ms)` tirado), cada um dos tres testes abaixo cai no seu: a
//! conexao anonima recebe `ok`, o leitor de `Z` trava `rh.salarios`, e o
//! prazo de 10^12 ms volta como `expira_em_s` de 10^9.
use super::*;

const SENHA: &str = "a-senha-forte-do-607";

fn servidor() -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo("607-escopo");
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{"token":"t","bind":"127.0.0.1:5399","base":"{}",
                    "usuarios":[{{"id":1,"nome":"Ana","login":"ana","senha_hash":"{}",
                    "supervisor":true}}]}}"#,
            dir.join("dados").display(),
            phxsql_core::senha::cifrar_com(SENHA, 64),
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let mut ana = sessao(&s, Some("ana"), 1);
    pede(&s, &mut ana, r#""op":"criar_database","database":"rh""#).unwrap();
    pede(
        &s,
        &mut ana,
        r#""op":"criar_tabela","database":"rh","tabela":"salarios",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    for (login, bases) in [
        ("leitor_z", r#"{"Z":{"ler":true}}"#),
        ("le_rh", r#"{"rh":{"ler":true}}"#),
        ("insere_rh", r#"{"rh":{"ler":true,"inserir":true}}"#),
        ("altera_rh", r#"{"rh":{"ler":true,"alterar":true}}"#),
    ] {
        pede(
            &s,
            &mut ana,
            &format!(
                r#""op":"usuario_criar","login":"{login}","senha":"{SENHA}",
                       "nome":"{login}","bases":{bases}"#
            ),
        )
        .unwrap();
    }
    (s, dir)
}

fn sessao(s: &Servidor, login: Option<&str>, ligacao: u64) -> Sessao {
    Sessao {
        usuario: login.and_then(|l| s.cadastro().por_login(l).cloned()),
        ligacao,
        ..Sessao::default()
    }
}

fn pede(s: &Arc<Servidor>, ses: &mut Sessao, corpo: &str) -> Result<Json> {
    let (_, _, r) = s.despachar(&format!(r#"{{"token":"t",{corpo}}}"#), ses, "");
    r
}

const TRAVA_TUDO: &str = r#""op":"begin","database":"rh","scope":["salarios"],
        "lock_mode":"EXCLUSIVE","timeout_ms":1000000000000"#;

/// Sem login: o `begin` com escopo cai no «faca login» -- para a tabela
/// que existe e para a que nao existe, com a MESMA frase. E o `begin` sem
/// escopo, que nao trava nada na abertura, continua anonimo.
#[test]
fn sem_login_o_escopo_recusa_e_nao_enumera() {
    let (s, _g) = servidor();
    let faca_login = s.msg("erro.faca_login", &[]);
    let mut anonima = sessao(&s, None, 10);
    let existe = pede(&s, &mut anonima, TRAVA_TUDO).unwrap_err();
    assert!(matches!(existe, PhxError::Autorizacao(_)), "{existe}");
    assert!(existe.to_string().contains(&faca_login), "{existe}");
    let nao_existe = pede(
        &s,
        &mut anonima,
        r#""op":"begin","database":"rh","scope":["nao_existe"]"#,
    )
    .unwrap_err();
    assert_eq!(existe.to_string(), nao_existe.to_string());

    // E nada ficou travado: o supervisor grava.
    let mut ana = sessao(&s, Some("ana"), 11);
    pede(
        &s,
        &mut ana,
        r#""op":"inserir","database":"rh","tabela":"salarios","linha":{"id":1}"#,
    )
    .unwrap();

    // O comportamento velho: `begin` sem escopo segue anonimo.
    pede(&s, &mut anonima, r#""op":"begin","database":"rh""#).unwrap();
    pede(&s, &mut anonima, r#""op":"rollback""#).unwrap();
}

/// Com login e SEM direito: a recusa do portao, nunca a SP000006 de um
/// conflito de trava, e a mesma frase para a tabela que existe e para a
/// que nao existe. A regua do direito e a do PostgreSQL: `EXCLUSIVE` pede
/// alterar (ou excluir); `AUTO` pede inserir, alterar ou excluir.
#[test]
fn o_escopo_confere_o_direito_de_cada_tabela() {
    let (s, _g) = servidor();
    let tentar = |login: &str, ligacao: u64, corpo: &str| {
        let mut ses = sessao(&s, Some(login), ligacao);
        let r = pede(&s, &mut ses, corpo);
        if r.is_ok() {
            pede(&s, &mut ses, r#""op":"rollback""#).unwrap();
        }
        r
    };
    let e = tentar("leitor_z", 20, TRAVA_TUDO).unwrap_err();
    assert!(matches!(e, PhxError::Autorizacao(_)), "{e}");
    assert!(e.to_string().contains("rh.salarios"), "{e}");
    let fantasma = tentar(
        "leitor_z",
        21,
        r#""op":"begin","database":"rh","scope":["fantasma"],"lock_mode":"EXCLUSIVE""#,
    )
    .unwrap_err();
    assert_eq!(
        e.to_string().replace("salarios", "X"),
        fantasma.to_string().replace("fantasma", "X"),
        "a recusa distingue a tabela que existe da que nao existe"
    );

    let auto = r#""op":"begin","database":"rh","scope":["salarios"]"#;
    // So ler nao trava nem a intencao.
    assert!(tentar("le_rh", 22, auto).is_err());
    assert!(tentar("le_rh", 23, TRAVA_TUDO).is_err());
    // Inserir trava a intencao, e nao a tabela inteira.
    tentar("insere_rh", 24, auto).unwrap();
    assert!(tentar("insere_rh", 25, TRAVA_TUDO).is_err());
    // Alterar trava as duas.
    tentar("altera_rh", 26, auto).unwrap();
    tentar("altera_rh", 27, TRAVA_TUDO).unwrap();
}

/// O prazo pedido acima do `transacao_prazo_min` vira o teto; pedido
/// abaixo vale como veio.
#[test]
fn o_prazo_da_transacao_tem_o_teto_do_config() {
    let (s, _g) = servidor();
    let teto_s = s.config.recursos.transacao_prazo_min * 60;
    let mut ana = sessao(&s, Some("ana"), 30);
    let r = pede(&s, &mut ana, TRAVA_TUDO).unwrap();
    let expira = r.inteiro_ou("expira_em_s", -1);
    assert!(
        (0..=teto_s as i64).contains(&expira),
        "pediu 10^12 ms e ganhou {expira} s: {}",
        r.escrever()
    );
    pede(&s, &mut ana, r#""op":"rollback""#).unwrap();
    let r = pede(
        &s,
        &mut ana,
        r#""op":"begin","database":"rh","timeout_ms":2000"#,
    )
    .unwrap();
    assert!(r.inteiro_ou("expira_em_s", -1) <= 2, "{}", r.escrever());
    pede(&s, &mut ana, r#""op":"rollback""#).unwrap();
}
