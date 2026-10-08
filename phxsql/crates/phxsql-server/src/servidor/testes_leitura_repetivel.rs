//! Leitura repetivel PELA TRAVA, pedida (`docs/SOMBRA.md` §5b) -- a prova
//! real nos dois sentidos, no molde da §7 daquele documento. Duas conexoes
//! de verdade (ligacoes 1 e 2), pelo `despachar`: e ali que mora o portao
//! da escrita comum (`portoes_do_pedido` -> `barrado_por_travas`), e um
//! teste que chamasse `executar` direto pularia exatamente a barreira que
//! quer provar -- foi o primeiro erro desta prova.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Base `b`, tabela `nums` (id unico, valor), uma linha `{1, 50}`.
fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"nums",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"valor","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"nums","linha":{"id":1,"valor":50}}"#),
        &dono,
    )
    .unwrap();
    s
}

fn conexao(ligacao: u64) -> Sessao {
    Sessao {
        ligacao,
        ..Sessao::default()
    }
}

/// Um pedido pela porta da frente, com o token do servidor.
fn manda(s: &Arc<Servidor>, ses: &mut Sessao, corpo: &str) -> Result<Json> {
    let (_, _, r) = s.despachar(&format!(r#"{{"token":"t",{corpo}}}"#), ses, "127.0.0.1");
    r
}

fn valor(s: &Arc<Servidor>, ses: &mut Sessao) -> i64 {
    manda(
        s,
        ses,
        r#""op":"ler","database":"b","tabela":"nums","rowid":1"#,
    )
    .unwrap()
    .inteiro_ou("valor", -1)
}

fn grava_77(s: &Arc<Servidor>, ses: &mut Sessao) -> Result<Json> {
    manda(
        s,
        ses,
        r#""op":"atualizar","database":"b","tabela":"nums","rowid":1,
               "valores":{"id":1,"valor":77}"#,
    )
}

fn insere_2(s: &Arc<Servidor>, ses: &mut Sessao) -> Result<Json> {
    manda(
        s,
        ses,
        r#""op":"inserir","database":"b","tabela":"nums","linha":{"id":2,"valor":9}"#,
    )
}

fn conta(s: &Arc<Servidor>, ses: &mut Sessao) -> i64 {
    manda(
        s,
        ses,
        r#""op":"coletar_rowids","database":"b","tabela":"nums""#,
    )
    .unwrap()
    .inteiro_ou("casaram", -1)
}

/// O CONTROLE, e o teste que tem de existir primeiro: *guarda nova entra
/// pedida, nao imposta*. Sem pedir, a transacao continua em READ
/// COMMITTED e a leitura NAO e repetivel -- e exatamente o fenomeno que o
/// `ACID.md` §4.1 mede acontecendo (50 -> 77). Se este teste mudasse, a
/// guarda teria sido imposta a quem nao pediu.
#[test]
fn sem_pedir_a_leitura_continua_nao_repetivel() {
    let dir = DirTemp::novo("repetivel-controle");
    let s = servidor(dir.as_ref());
    let mut a = conexao(1);
    let mut b = conexao(2);

    manda(&s, &mut a, r#""op":"begin","database":"b""#).unwrap();
    assert_eq!(valor(&s, &mut a), 50);
    grava_77(&s, &mut b).expect("sem leitura repetivel, o escritor NAO espera");
    assert_eq!(
        valor(&s, &mut a),
        77,
        "sem pedir, a segunda leitura ve o commit alheio: e READ COMMITTED"
    );
    manda(&s, &mut a, r#""op":"commit","database":"b""#).unwrap();
}

/// PROVA REAL da leitura repetivel: quem PEDE segura a S; o escritor e
/// barrado (o autocommit recebe `EM_TRANSACAO` nomeando quem segura); a
/// releitura devolve o MESMO 50; o COMMIT solta a S e o escritor entra.
///
/// Com o portao desligado, a escrita de `b` passaria no meio e a segunda
/// leitura devolveria 77 -- o teste tem de ficar vermelho.
#[test]
fn pedindo_a_leitura_e_repetivel_e_o_escritor_espera() {
    let dir = DirTemp::novo("repetivel-liga");
    let s = servidor(dir.as_ref());
    let mut a = conexao(1);
    let mut b = conexao(2);

    manda(
        &s,
        &mut a,
        r#""op":"begin","database":"b","leitura_repetivel":true"#,
    )
    .unwrap();
    assert_eq!(valor(&s, &mut a), 50, "primeira leitura toma a S");

    let e = grava_77(&s, &mut b).expect_err("o escritor tinha de esperar o leitor");
    assert!(
        matches!(e, PhxError::EmTransacao(_)),
        "a recusa nomeia a transacao que segura a S: {e}"
    );

    assert_eq!(
        valor(&s, &mut a),
        50,
        "a releitura devolve o MESMO estado: leitura repetivel"
    );

    // A ficha diz o nivel que esta valendo, e nao o de sempre.
    let f = manda(&s, &mut a, r#""op":"transacao","database":"b""#).unwrap();
    assert!(
        f.booleano_ou("leitura_repetivel", false),
        "{}",
        f.escrever()
    );
    assert!(
        f.texto_ou("transaction_isolation", "")
            .contains("repetivel pela trava"),
        "{}",
        f.texto_ou("transaction_isolation", "")
    );

    manda(&s, &mut a, r#""op":"commit","database":"b""#).unwrap();
    grava_77(&s, &mut b).expect("solta a S, o escritor entra");
    assert_eq!(valor(&s, &mut conexao(3)), 77);
}

/// SEM FANTASMA, e de graca: sob a S nenhum `INSERT` entra (o fim da
/// tabela e disputado como uma linha), entao a mesma varredura repetida
/// devolve o MESMO numero de linhas -- o 2 -> 3 do `ACID.md` nao acontece
/// para quem pediu. A Sombra precisava de um filtro de nascimento para
/// isto; a trava nao precisa de nada.
#[test]
fn pedindo_nao_ha_fantasma() {
    let dir = DirTemp::novo("repetivel-fantasma");
    let s = servidor(dir.as_ref());
    let mut a = conexao(1);
    let mut b = conexao(2);

    manda(
        &s,
        &mut a,
        r#""op":"begin","database":"b","leitura_repetivel":true"#,
    )
    .unwrap();
    assert_eq!(conta(&s, &mut a), 1);

    let e = insere_2(&s, &mut b).expect_err("o INSERT alheio tinha de esperar o leitor");
    assert!(matches!(e, PhxError::EmTransacao(_)), "{e}");

    assert_eq!(
        conta(&s, &mut a),
        1,
        "a varredura repetida devolve o mesmo conjunto"
    );

    manda(&s, &mut a, r#""op":"commit","database":"b""#).unwrap();
    insere_2(&s, &mut b).expect("solta a S, o INSERT entra");
    assert_eq!(conta(&s, &mut conexao(3)), 2);
}

/// A RECUSA E DO LEITOR (§7 do SOMBRA): quem pede leitura repetivel numa
/// tabela que outra transacao esta escrevendo espera o `LOCK TIMEOUT` e
/// recebe o erro -- o escritor nao perde a vazao por causa dele.
#[test]
fn a_recusa_e_do_leitor_que_pediu() {
    let dir = DirTemp::novo("repetivel-recusa-do-leitor");
    let s = servidor(dir.as_ref());
    let mut escritor = conexao(1);
    let mut leitor = conexao(2);

    manda(&s, &mut escritor, r#""op":"begin","database":"b""#).unwrap();
    grava_77(&s, &mut escritor).expect("empilha e toma a linha 1");

    manda(
        &s,
        &mut leitor,
        r#""op":"begin","database":"b","leitura_repetivel":true,"lock_timeout_ms":20"#,
    )
    .unwrap();
    let e = manda(
        &s,
        &mut leitor,
        r#""op":"ler","database":"b","tabela":"nums","rowid":1"#,
    )
    .expect_err("a S tinha de esperar a linha presa e desistir no LOCK TIMEOUT");
    assert!(
        matches!(e, PhxError::EmTransacao(_)),
        "a recusa e do leitor, com o LOCK TIMEOUT no texto: {e}"
    );
    assert!(e.to_string().contains("LOCK TIMEOUT"), "{e}");

    // O escritor nem sentiu.
    manda(&s, &mut escritor, r#""op":"commit","database":"b""#).unwrap();
    manda(&s, &mut leitor, r#""op":"rollback","database":"b""#).unwrap();
    assert_eq!(valor(&s, &mut conexao(3)), 77);
}

/// DUAS que pediram leitura repetivel e leram a MESMA tabela nao se
/// atropelam caladas: a S e compartilhada entre leitores, mas a escrita
/// de uma sobre o que a outra LEU espera a S da outra e desiste no LOCK
/// TIMEOUT -- nos dois sentidos, porque nao ha detector de impasse e o
/// prazo e quem resolve. Sob S de tabela, o skew de escrita do `ACID.md`
/// §4.4 (le x, escreve y) e a perda de atualizacao viram o mesmo caso:
/// escrever o que a outra leu. NAO se reivindica SERIALIZABLE por isto;
/// o que se afirma e o que este teste mede.
#[test]
fn duas_repetiveis_que_leram_a_mesma_tabela_nao_se_atropelam() {
    let dir = DirTemp::novo("repetivel-duas");
    let s = servidor(dir.as_ref());
    let mut a = conexao(1);
    let mut b = conexao(2);
    let abre = r#""op":"begin","database":"b","leitura_repetivel":true,"lock_timeout_ms":20"#;
    manda(&s, &mut a, abre).unwrap();
    manda(&s, &mut b, abre).unwrap();
    assert_eq!(valor(&s, &mut a), 50, "leitores compartilham a S");
    assert_eq!(valor(&s, &mut b), 50, "leitores compartilham a S");

    let ea = grava_77(&s, &mut a).expect_err("a S de b barra a escrita de a");
    assert!(matches!(ea, PhxError::EmTransacao(_)), "{ea}");
    assert!(ea.to_string().contains("LOCK TIMEOUT"), "{ea}");
    let eb = grava_77(&s, &mut b).expect_err("a S de a barra a escrita de b");
    assert!(matches!(eb, PhxError::EmTransacao(_)), "{eb}");
    assert!(eb.to_string().contains("LOCK TIMEOUT"), "{eb}");

    manda(&s, &mut a, r#""op":"rollback","database":"b""#).unwrap();
    manda(&s, &mut b, r#""op":"rollback","database":"b""#).unwrap();
    assert_eq!(valor(&s, &mut conexao(3)), 50, "nada foi gravado");
}

/// PELO SQL, de ponta a ponta: `BEGIN ISOLATION LEVEL REPEATABLE READ` e
/// um `SELECT` -- que nao tem campo `tabela` nenhum e vira `varrer`/
/// `buscar` derivados -- tomam a S do mesmo jeito, porque o
/// `executar_derivado` e irmao do `despachar` e passa pelo mesmo portao.
/// E a prova de que o gancho esta no lugar unico, e nao num `op_ler`.
#[test]
fn pelo_sql_o_select_tambem_toma_a_s() {
    let dir = DirTemp::novo("repetivel-sql");
    let s = servidor(dir.as_ref());
    let mut a = conexao(1);
    let mut b = conexao(2);

    manda(
        &s,
        &mut a,
        r#""op":"sql","database":"b","texto":"BEGIN ISOLATION LEVEL REPEATABLE READ LOCK TIMEOUT 20ms""#,
    )
    .expect("a abertura pelo SQL");
    let r = manda(
        &s,
        &mut a,
        r#""op":"sql","database":"b","texto":"SELECT valor FROM nums WHERE id = 1""#,
    )
    .expect("o SELECT dentro da transacao");
    assert!(r.escrever().contains("50"), "{}", r.escrever());

    let e = grava_77(&s, &mut b).expect_err("o SELECT tomou a S; o escritor solto e barrado");
    assert!(matches!(e, PhxError::EmTransacao(_)), "{e}");

    manda(&s, &mut a, r#""op":"sql","database":"b","texto":"COMMIT""#).unwrap();
    grava_77(&s, &mut b).expect("o COMMIT soltou a S");
    assert_eq!(valor(&s, &mut conexao(3)), 77);
}
