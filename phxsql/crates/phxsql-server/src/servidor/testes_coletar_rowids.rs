use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um servidor com a base `b` e a tabela `nums` (id, valor), com `linhas`
/// linhas cujo `valor` vai de 1 a `linhas`.
fn servidor(dir: &std::path::Path, linhas: i64) -> (Arc<Servidor>, Sessao) {
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
    for i in 1..=linhas {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"nums","linha":{{"id":{i},"valor":{i}}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    (s, dono)
}

/// Prova real nos dois sentidos: `coletar_rowids` com `valor > 3` numa
/// tabela de cinco devolve EXATAMENTE as duas que casam, e sem filtro
/// devolve as cinco. Se o `passa` deixasse de peneirar, o `casaram == 2`
/// falharia (viriam as cinco).
#[test]
fn coleta_exatamente_as_linhas_que_casam() {
    let dir = DirTemp::novo("coletar-casam");
    let (s, dono) = servidor(dir.as_ref(), 5);

    let r = s
        .executar(
            "coletar_rowids",
            &pedido(
                r#"{"database":"b","tabela":"nums",
                        "onde":[{"coluna":"valor","op":">","valor":3}]}"#,
            ),
            &dono,
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("casaram", -1), 2, "valor>3 casa duas linhas");
    assert_eq!(r.inteiro_ou("examinadas", -1), 5, "examinou as cinco");
    assert_eq!(r.campo("rowids").and_then(Json::lista).unwrap().len(), 2);

    let todas = s
        .executar(
            "coletar_rowids",
            &pedido(r#"{"database":"b","tabela":"nums"}"#),
            &dono,
        )
        .unwrap();
    assert_eq!(todas.inteiro_ou("casaram", -1), 5, "sem filtro, todas");
}

/// Prova real do teto: baixado a 3 sobre uma tabela de cinco, o coletar
/// RECUSA nomeando o limite, em vez de responder sobre as tres primeiras
/// com cara de ter respondido inteiro. Se ele truncasse, viria Ok e o
/// `expect_err` entraria em panico.
#[test]
fn passar_do_teto_recusa_em_vez_de_truncar() {
    let dir = DirTemp::novo("coletar-teto");
    let (s, dono) = servidor(dir.as_ref(), 5);

    let e = s
        .executar(
            "coletar_rowids",
            &pedido(r#"{"database":"b","tabela":"nums","teto":3}"#),
            &dono,
        )
        .expect_err("passou do teto e nao recusou");
    assert!(
        format!("{e}").contains('3'),
        "a recusa tem de nomear o limite: {e}"
    );

    let ok = s
        .executar(
            "coletar_rowids",
            &pedido(r#"{"database":"b","tabela":"nums","teto":5}"#),
            &dono,
        )
        .unwrap();
    assert_eq!(
        ok.inteiro_ou("casaram", -1),
        5,
        "com teto suficiente, passa"
    );
}

/// Quantas linhas ATIVAS casam este filtro -- a sonda que mede o efeito de
/// um UPDATE/DELETE por faixa sem depender de ler rowid por rowid.
fn casaram(s: &Arc<Servidor>, dono: &Sessao, onde: &str) -> i64 {
    let corpo = if onde.is_empty() {
        r#"{"database":"b","tabela":"nums"}"#.to_string()
    } else {
        format!(r#"{{"database":"b","tabela":"nums","onde":[{onde}]}}"#)
    };
    s.executar("coletar_rowids", &pedido(&corpo), dono)
        .unwrap()
        .inteiro_ou("casaram", -1)
}

/// PROVA REAL do UPDATE por faixa, nos dois sentidos: `UPDATE ... WHERE
/// valor > 3` numa tabela de cinco muda EXATAMENTE as duas que casam
/// (`afetadas == 2`), as duas passam a ter `valor = 0`, e as tres de baixo
/// ficam intactas. Se o laco de `executar_dml_por_faixa` processasse so' a
/// PRIMEIRA linha colhida -- o `find` no lugar do `filter` que esta casa ja
/// pagou --, `afetadas` viria 1 e so' uma linha mudaria: as duas asserts de
/// baixo pegam os dois lados do defeito.
#[test]
fn update_por_faixa_muda_todas_as_que_casam_e_so_elas() {
    let dir = DirTemp::novo("faixa-update");
    let (s, dono) = servidor(dir.as_ref(), 5);

    let r = s
        .executar(
            "sql",
            &pedido(r#"{"database":"b","texto":"UPDATE nums SET valor = 0 WHERE valor > 3"}"#),
            &dono,
        )
        .unwrap();
    assert_eq!(
        r.inteiro_ou("afetadas", -1),
        2,
        "duas linhas casam valor>3 -- se so' a primeira fosse tratada, viria 1: {}",
        r.escrever()
    );

    // As duas que casavam agora valem 0, e nenhuma outra: sao duas medidas,
    // porque um laco que gravasse 0 em TODAS tambem daria `valor = 0` em
    // duas... nao, daria em cinco. A conta fecha so' se mudou as certas.
    assert_eq!(
        casaram(&s, &dono, r#"{"coluna":"valor","op":"=","valor":0}"#),
        2,
        "duas linhas ficaram com valor 0"
    );
    assert_eq!(
        casaram(&s, &dono, r#"{"coluna":"valor","op":">","valor":3}"#),
        0,
        "nenhuma linha ainda casa valor>3"
    );
    assert_eq!(
        casaram(&s, &dono, r#"{"coluna":"valor","op":"=","valor":1}"#),
        1,
        "a linha de baixo (valor 1) ficou intacta"
    );
}

/// PROVA REAL do DELETE por faixa: `DELETE FROM nums WHERE valor <= 2`
/// apaga EXATAMENTE as duas de baixo (`afetadas == 2`), elas somem da
/// visao ativa e sobram tres. E o excluir SUAVE por linha -- a mesma
/// medida que o `find` em vez do `filter` derrubaria para 1.
#[test]
fn delete_por_faixa_apaga_todas_as_que_casam() {
    let dir = DirTemp::novo("faixa-delete");
    let (s, dono) = servidor(dir.as_ref(), 5);

    let r = s
        .executar(
            "sql",
            &pedido(r#"{"database":"b","texto":"DELETE FROM nums WHERE valor <= 2"}"#),
            &dono,
        )
        .unwrap();
    assert_eq!(
        r.inteiro_ou("afetadas", -1),
        2,
        "duas linhas casam valor<=2: {}",
        r.escrever()
    );

    assert_eq!(
        casaram(&s, &dono, ""),
        3,
        "sobraram tres linhas ativas apos o DELETE por faixa"
    );
    assert_eq!(
        casaram(&s, &dono, r#"{"coluna":"valor","op":"<=","valor":2}"#),
        0,
        "nenhuma linha ativa ainda casa valor<=2"
    );
}

/// Zero linhas nao e erro: um UPDATE por faixa cujo filtro nao casa nada
/// devolve `afetadas: 0` e Ok, como todo o resto do SQL -- e nao um erro,
/// que seria a resposta errada calada.
#[test]
fn por_faixa_que_nao_casa_nada_e_zero_e_nao_erro() {
    let dir = DirTemp::novo("faixa-zero");
    let (s, dono) = servidor(dir.as_ref(), 5);

    let r = s
        .executar(
            "sql",
            &pedido(r#"{"database":"b","texto":"UPDATE nums SET valor = 9 WHERE valor > 100"}"#),
            &dono,
        )
        .expect("filtro que nao casa nada nao e erro");
    assert_eq!(
        r.inteiro_ou("afetadas", -1),
        0,
        "zero afetadas: {}",
        r.escrever()
    );
    assert_eq!(
        casaram(&s, &dono, r#"{"coluna":"valor","op":"=","valor":9}"#),
        0,
        "nada foi gravado"
    );
}
