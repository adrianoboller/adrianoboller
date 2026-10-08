//! Pedido 458: o `SP000010` nomeia a trava, e um panico com as transacoes na
//! mao deixa de matar toda transacao de toda conexao ate reiniciar.
use super::*;

fn servidor(dir: &Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

fn sessao(ligacao: u64) -> Sessao {
    Sessao {
        ligacao,
        ip: "127.0.0.1".into(),
        ..Sessao::default()
    }
}

fn pede(s: &Arc<Servidor>, ligacao: u64, corpo: &str) -> Result<Json> {
    let mut ses = sessao(ligacao);
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

fn ids(s: &Arc<Servidor>, tabela: &str) -> Vec<i64> {
    let r = pede(
        s,
        1,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":100"#),
    )
    .unwrap();
    let mut v: Vec<i64> = r
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect();
    v.sort();
    v
}

fn inserir(tabela: &str, id: i64) -> String {
    format!(r#""op":"inserir","database":"loja","tabela":"{tabela}","linha":{{"id":{id}}}"#)
}

/// **O dano do achado, e o saneamento, na mesma corrida.**
///
/// A conexao 7 abre transacao e empilha o id 1. Um panico cai com o
/// registro das transacoes na mao. Antes: toda tomada seguinte recusava
/// com `SP000010`, e a conexao 8 nao conseguia nem abrir transacao ate o
/// servidor reiniciar. Agora a 8 abre, grava e confirma -- e a 7, cujo
/// conjunto de escrita o registro nao pode afirmar inteiro, NAO confirma:
/// vai para `ABORT_ONLY` e so o ROLLBACK passa. Recuperar sem sanear
/// deixaria o COMMIT da 7 gravar o que o panico pode ter cortado ao meio.
///
/// A 8 grava em OUTRA tabela de proposito: a 7 abortada segura a trava
/// de `clientes` ate o ROLLBACK, a queda ou o prazo. Isso e escolha NOSSA,
/// a mesma do `ABORT_ONLY` por erro de transacao -- o PostgreSQL(R) solta
/// as travas ja no abort, medido pelo papel C no PG 16.13. E o saneamento
/// nao as solta porque roda com `transacoes` na mao, e tomar `travas` dali
/// inverteria a ordem do `barrado_por_travas`. Quem espera por ela e a 9,
/// que so entra depois do ROLLBACK.
#[test]
fn o_panico_com_as_transacoes_na_mao_nao_mata_a_proxima() {
    let dir = DirTemp::novo("trava-suja-transacoes");
    let s = servidor(&dir);
    pede(&s, 1, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        &s,
        1,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    pede(
        &s,
        1,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();

    pede(&s, 7, r#""op":"begin""#).unwrap();
    pede(&s, 7, &inserir("clientes", 1)).unwrap();

    s.transacoes.envenenar();

    pede(&s, 8, r#""op":"begin""#)
        .unwrap_or_else(|e| panic!("a transacao NOVA morreu pelo panico alheio: {e}"));
    pede(&s, 8, &inserir("pedidos", 2)).unwrap();
    pede(&s, 8, r#""op":"commit""#).unwrap();
    assert_eq!(ids(&s, "pedidos"), vec![2]);

    let e = pede(&s, 7, r#""op":"commit""#)
        .expect_err("a transacao aberta no panico confirmou o que o registro nao afirma");
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA", "{e}");
    pede(&s, 7, r#""op":"rollback""#).unwrap();
    assert!(
        ids(&s, "clientes").is_empty(),
        "o id 1 da transacao saneada foi gravado"
    );

    // O veneno do `Mutex` nao sai: a terceira transacao continua servida,
    // agora na tabela que a 7 soltou no ROLLBACK.
    pede(&s, 9, r#""op":"begin""#).unwrap();
    pede(&s, 9, &inserir("clientes", 3)).unwrap();
    pede(&s, 9, r#""op":"commit""#).unwrap();
    assert_eq!(ids(&s, "clientes"), vec![3]);
}

/// **O SEGUNDO panico tambem saneia** -- C3 do parecer do DBA aos faceis
/// C. E a razao de a `Tomada` existir: o veneno do `Mutex` nao sai, e o
/// aviso «uma vez por trava» de antes (`veneno_dito`) passaria calado pelo
/// segundo panico -- e, com o saneamento no meio, sem saneamento. A 7 abre
/// e o primeiro panico a saneia; depois do ROLLBACK dela, a 8 abre e
/// empilha, cai o segundo panico, e o COMMIT da 8 tem de recusar.
#[test]
fn o_segundo_panico_com_as_transacoes_na_mao_tambem_saneia() {
    let dir = DirTemp::novo("trava-suja-duas-vezes");
    let s = servidor(&dir);
    pede(&s, 1, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        &s,
        1,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();

    pede(&s, 7, r#""op":"begin""#).unwrap();
    s.transacoes.envenenar();
    let e = pede(&s, 7, r#""op":"commit""#).expect_err("o primeiro panico nao saneou");
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA", "{e}");
    pede(&s, 7, r#""op":"rollback""#).unwrap();

    pede(&s, 8, r#""op":"begin""#).unwrap();
    pede(&s, 8, &inserir("clientes", 1)).unwrap();
    s.transacoes.envenenar();
    let e = pede(&s, 8, r#""op":"commit""#)
        .expect_err("o SEGUNDO panico passou sem saneamento: a 8 confirmou");
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA", "{e}");
    pede(&s, 8, r#""op":"rollback""#).unwrap();
    assert!(
        ids(&s, "clientes").is_empty(),
        "o id 1 da transacao do segundo panico foi gravado"
    );
}

/// Onde ha disco atras a trava suja continua RECUSANDO -- recuperar as
/// cegas serviria estado que ninguem afirma --, mas agora diz QUAL: a
/// frase era a mesma em 85 pontos de 14 travas.
#[test]
fn a_trava_com_disco_atras_recusa_dizendo_qual() {
    let dir = DirTemp::novo("trava-suja-visoes");
    let s = servidor(&dir);
    let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _na_mao = s.visoes.lock();
        panic!("panico de proposito, com a trava das visoes na mao");
    }));
    assert!(morreu.is_err() && s.visoes.is_poisoned());
    for _ in 0..2 {
        let e = pede(&s, 1, r#""op":"visoes","database":"loja""#)
            .expect_err("a trava suja com disco atras passou a servir as cegas");
        let t = e.to_string();
        assert!(t.starts_with("[SP000010]"), "{t}");
        assert!(t.contains("\"visoes\""), "a recusa nao nomeia a trava: {t}");
    }
}
