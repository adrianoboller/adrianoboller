//! O SQL COMPOSTO ponta a ponta: o texto entra pela op `sql`, a F-SQL o
//! traduz para `consultar`/`agrupar`, e o servidor o executa.
//!
//! # Por que estes testes moram aqui, e nao no crate de SQL
//!
//! La se prova que o TEXTO vira o PEDIDO certo -- e isso ja esta provado la.
//! O que so se prova aqui e que o pedido, executado, devolve o DADO certo:
//! nenhum teste de tradução acusa uma junção que casa pela coluna errada, e
//! nenhum teste de execucao acusa um `ON` traduzido ao contrario. A costura
//! e o que so aparece no encontro das duas frentes.
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("sqlc-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn servidor(d: &std::path::Path, cadastro: Cadastro) -> (Arc<Servidor>, Sessao) {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        cadastro: cadastro.clone(),
        max_linhas: 10_000,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let ses = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            // `porCidade` existe porque o tradutor de SQL EXIGE indice
            // para um `WHERE coluna = literal`: sem ele, um SELECT
            // responderia sobre a primeira pagina com a cara de ter
            // respondido sobre a tabela. Ver `docs/SQL.md`.
            r#"{"database":"b","tabela":"clientes","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"},
                    {"nome":"cidade","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                            {"nome":"porCidade","colunas":["cidade"]}]}"#,
        ),
        &ses,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"pedidos","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"cliente_id","tipo":"Int4"},
                    {"nome":"total","tipo":"Int4"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"folha","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    for (id, nome, cidade) in [
        (1, "ana", "Blumenau"),
        (2, "bia", "Itajai"),
        (3, "caio", "Blumenau"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"clientes",
                         "linha":{{"id":{id},"nome":"{nome}","cidade":"{cidade}"}}}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    for (id, cli, total) in [(1, 1, 100), (2, 1, 50), (3, 2, 300), (4, 9, 7)] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"pedidos",
                         "linha":{{"id":{id},"cliente_id":{cli},"total":{total}}}}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"folha","linha":{"id":1,"nome":"segredo"}}"#),
        &ses,
    )
    .unwrap();
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

fn sql(s: &Arc<Servidor>, texto: &str) -> Result<Json> {
    s.executar(
        "sql",
        &pedido(&format!(
            r#"{{"database":"b","texto":{}}}"#,
            Json::texto_de(texto).escrever()
        )),
        &Sessao::default(),
    )
}

/// **O `sql` responde UM `colunas` -- a lista de rotulos --, mesmo agora
/// que o `consultar` de dentro responde o modelo tipado no mesmo nome.**
///
/// Com o `continue` de `resposta_do_sql` fora, a chave sairia DUAS vezes
/// no objeto, com formas diferentes, e quem le veria uma sem saber qual.
#[test]
fn o_sql_nao_sai_com_colunas_em_dobro() {
    let d = dir("colunas-uma-vez");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = sql(
        &s,
        "SELECT p.id, c.nome FROM pedidos p JOIN clientes c ON p.cliente_id = c.id",
    )
    .unwrap();
    let quantas = r.chaves().iter().filter(|k| **k == "colunas").count();
    assert_eq!(quantas, 1, "{}", r.escrever());
    let col = r.campo("colunas").and_then(Json::lista).unwrap();
    assert!(
        col.iter().all(|c| c.texto().is_some()),
        "o modelo tipado vazou para o sql: {}",
        r.escrever()
    );
    let _ = std::fs::remove_dir_all(&d);
}

fn linhas(r: &Json) -> Vec<Json> {
    r.campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .to_vec()
}

/// **INNER JOIN ponta a ponta, e a prova mede o DADO.**
///
/// Sao quatro pedidos e tres clientes, e o pedido 4 aponta para um
/// cliente que nao existe: o INNER traz tres linhas, e cada uma com o nome
/// do cliente CERTO. Um `ON` traduzido ao contrario tambem traria tres
/// linhas -- e por isso o teste confere o par, e nao a contagem.
#[test]
fn o_inner_join_junta_pelo_par_certo() {
    let d = dir("join");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = sql(
        &s,
        "SELECT p.id, c.nome AS cliente FROM pedidos p \
             JOIN clientes c ON p.cliente_id = c.id",
    )
    .expect("o JOIN nao rodou");
    let l = linhas(&r);
    assert_eq!(l.len(), 3, "o pedido orfao entrou");
    // O PAR, e nao so a contagem: um `ON` traduzido ao contrario traria
    // tres linhas tambem, com o cliente errado em cada uma.
    let pares: Vec<(i64, String)> = l
        .iter()
        .map(|x| {
            (
                x.inteiro_ou("p.id", -1),
                x.texto_ou("cliente", "").to_string(),
            )
        })
        .collect();
    assert_eq!(
        pares,
        vec![
            (1, "ana".to_string()),
            (2, "ana".to_string()),
            (3, "bia".to_string())
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// LEFT JOIN mantem o orfao, com o lado de fora nulo.
#[test]
fn o_left_join_mantem_o_orfao() {
    let d = dir("left");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = sql(
        &s,
        "SELECT p.id, c.nome AS cliente FROM pedidos p \
             LEFT JOIN clientes c ON p.cliente_id = c.id",
    )
    .unwrap();
    let l = linhas(&r);
    assert_eq!(l.len(), 4, "o LEFT perdeu o orfao");
    // O pedido 4 aponta para um cliente que nao existe: ele FICA, com o
    // lado de fora nulo -- que e o que faz o LEFT ser um LEFT.
    let orfao = l
        .iter()
        .find(|x| x.inteiro_ou("p.id", -1) == 4)
        .expect("o orfao sumiu");
    assert_eq!(orfao.campo("cliente"), Some(&Json::Nulo));
    let _ = std::fs::remove_dir_all(&d);
}

/// `GROUP BY` com `HAVING` ponta a ponta.
#[test]
fn o_group_by_com_having() {
    let d = dir("group");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = sql(
        &s,
        "SELECT cidade, COUNT(*) AS n FROM clientes GROUP BY cidade HAVING n > 1",
    )
    .expect("o GROUP BY nao rodou");
    let l = linhas(&r);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].texto_ou("cidade", ""), "Blumenau");
    assert_eq!(l[0].inteiro_ou("n", -1), 2);
    let _ = std::fs::remove_dir_all(&d);
}

/// `IN (SELECT …)` ponta a ponta.
#[test]
fn o_in_de_subconsulta_ponta_a_ponta() {
    let d = dir("in");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = sql(
        &s,
        "SELECT id FROM clientes WHERE id IN (SELECT cliente_id FROM pedidos) ORDER BY id",
    )
    .expect("o IN nao rodou");
    let l = linhas(&r);
    let ids: Vec<i64> = l.iter().map(|x| x.inteiro_ou("id", -1)).collect();
    assert_eq!(ids, vec![1, 2], "o cliente 3 nao tem pedido");
    let _ = std::fs::remove_dir_all(&d);
}

/// **`?` NO LEXICO, e nunca por substituicao de texto.**
///
/// A prova e a que importa numa defesa contra injecao: o parametro que
/// CONTEM SQL entra como VALOR e nao como comando. Se ele fosse colado no
/// texto antes da analise, `'; DROP TABLE clientes --` apagaria a tabela;
/// aqui ele so nao casa com cidade nenhuma, e as tres linhas continuam la.
#[test]
fn o_parametro_entra_no_lexico_e_nao_no_texto() {
    let d = dir("param");
    let (s, _) = servidor(&d, Cadastro::default());
    let com_parametros = |texto: &str, params: &str| -> Result<Json> {
        s.executar(
            "sql",
            &pedido(&format!(
                r#"{{"database":"b","texto":{},"parametros":{params}}}"#,
                Json::texto_de(texto).escrever()
            )),
            &Sessao::default(),
        )
    };

    let r = com_parametros("SELECT * FROM clientes WHERE id = ?", "[2]").unwrap();
    let l = linhas(&r);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].texto_ou("nome", ""), "bia");

    // O parametro HOSTIL: ele e um texto, e continua sendo um texto.
    // Cabe em `Str(20)` de proposito -- um payload maior seria recusado
    // pelo TAMANHO da chave, e a recusa esconderia o que o teste prova.
    let r = com_parametros(
        "SELECT * FROM clientes WHERE cidade = ?",
        r#"["';DROP TABLE x--"]"#,
    )
    .expect("o parametro hostil derrubou a consulta");
    assert_eq!(linhas(&r).len(), 0, "casou com alguma cidade?");
    // E a tabela continua inteira -- que e a prova de que nada foi colado.
    let r = sql(&s, "SELECT * FROM clientes").unwrap();
    assert_eq!(linhas(&r).len(), 3, "a tabela sumiu: houve injecao");

    // Contagem errada de `?` recusa dizendo quantos vieram.
    let e = com_parametros("SELECT * FROM clientes WHERE id = ? AND nome = ?", "[1]")
        .expect_err("aceitou parametro faltando");
    assert!(!e.to_string().is_empty(), "recusa muda nao ensina nada");
    let _ = std::fs::remove_dir_all(&d);
}

/// `CREATE VIEW` e `DROP VIEW` pelo SQL, e o `FROM` dela.
#[test]
fn o_create_view_pelo_sql_e_o_from_dela() {
    let d = dir("view");
    let (s, _) = servidor(&d, Cadastro::default());
    sql(&s, "CREATE VIEW v_todos AS SELECT * FROM clientes").expect("o CREATE VIEW nao rodou");
    let r = sql(&s, "SELECT nome FROM v_todos ORDER BY id DESC").unwrap();
    let l = linhas(&r);
    assert_eq!(l.len(), 3);
    assert_eq!(l[0].texto_ou("nome", ""), "caio");
    sql(&s, "DROP VIEW v_todos").expect("o DROP VIEW nao rodou");
    // Sumiu a visao, o FROM volta a procurar TABELA -- e nao acha.
    assert!(sql(&s, "SELECT * FROM v_todos").is_err());
    let _ = std::fs::remove_dir_all(&d);
}

/// **O SELECT COMPOSTO tambem NAO e a porta dos fundos.**
///
/// A tabela negada no lado de dentro de uma junção escrita em SQL recusa,
/// e o controle -- a junção entre as permitidas -- responde na mesma
/// corrida. O portao entra duas vezes: no planejamento (o `esquema` de
/// cada lado) e na execucao (cada sub-pedido do `consultar`).
#[test]
fn o_join_pelo_sql_nao_e_a_porta_dos_fundos() {
    let d = dir("porta");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&d, cadastro);
    let pede = |texto: &str| -> Result<Json> {
        let mut sessao = Sessao {
            usuario: ses.usuario.clone(),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(
                r#"{{"token":"t","op":"sql","database":"b","texto":{}}}"#,
                Json::texto_de(texto).escrever()
            ),
            &mut sessao,
            "127.0.0.1",
        );
        r
    };
    pede("SELECT p.id FROM pedidos p JOIN clientes c ON p.cliente_id = c.id")
        .expect("as tabelas permitidas foram barradas");
    let e = pede("SELECT p.id FROM pedidos p JOIN folha f ON p.id = f.id")
        .expect_err("o JOIN leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    // E pelo IN, que esconde a tabela um nivel mais fundo.
    let e = pede("SELECT id FROM clientes WHERE id IN (SELECT id FROM folha)")
        .expect_err("o IN leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
