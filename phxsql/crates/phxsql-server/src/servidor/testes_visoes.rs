//! As visoes pelo protocolo, e o `FROM v` da op `sql`.
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("visao-{rotulo}"))
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
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    for tab in ["clientes", "folha"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{tab}","colunas":[
                        {{"nome":"id","tipo":"Int4","obrigatoria":true}},
                        {{"nome":"nome","tipo":"Str(20)"}},
                        {{"nome":"cidade","tipo":"Str(20)"}}],
                     "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                  "primario":true}}]}}"#
            )),
            &dono,
        )
        .unwrap();
    }
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
            &dono,
        )
        .unwrap();
    }
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"folha","linha":{"id":1,"nome":"segredo"}}"#),
        &dono,
    )
    .unwrap();
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

fn dono(s: &Arc<Servidor>, op: &str, corpo: &str) -> Result<Json> {
    s.executar(op, &pedido(corpo), &Sessao::default())
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

fn linhas_do_sql(r: &Json) -> Vec<Json> {
    r.campo("linhas").and_then(Json::lista).unwrap().to_vec()
}

/// **CONTRATO, nao defeito** -- pedido 245, O6.
///
/// `SELECT * FROM v_ord` recusa quando a visao pede uma direcao que o
/// `.ndx` nao guarda. Isso parece defeito da visao e nao e: medido em
/// 16/09/2026, a recusa e **exatamente a mesma** do `SELECT` direto, letra
/// por letra. A visao nao piora nada -- ela repassa a regra do motor, que
/// e «a ordem sai do indice, e a direcao esta gravada nele».
///
/// Que o `CREATE VIEW` ACEITE e deliberado e esta escrito no
/// `visoes.rs`: a visao guarda TEXTO e e reanalisada a cada uso, para
/// falar de um esquema que envelhece. Compila-la na criacao a congelaria
/// contra a tabela de hoje.
///
/// Este teste e a guarda do contrato: se um dia as duas recusas
/// divergirem, ele cai -- e ai ha mesmo um defeito da visao para achar.
#[test]
fn a_visao_recusa_a_direcao_com_a_mesma_frase_do_select_direto() {
    let d = dir("ordem");
    let (s, _) = servidor(&d, Cadastro::default());
    // Que o CREATE aceite e a decisao do `visoes.rs`, e nao um descuido.
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_ord",
                "sql":"SELECT * FROM clientes ORDER BY id DESC"}"#,
    )
    .unwrap();

    let pela_visao = sql(&s, "SELECT * FROM v_ord").unwrap_err().to_string();
    let direto = sql(&s, "SELECT * FROM clientes ORDER BY id DESC")
        .unwrap_err()
        .to_string();
    assert_eq!(pela_visao, direto, "a visao divergiu do caminho direto");
    assert!(pela_visao.contains("gravada no .ndx"), "{pela_visao}");
    // E a saida vai na mensagem: onde se declara a outra direcao.
    assert!(pela_visao.contains("criacao da tabela"), "{pela_visao}");

    // O CONTROLE: no sentido que o indice guarda, a visao responde.
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_asc",
                "sql":"SELECT * FROM clientes ORDER BY id"}"#,
    )
    .unwrap();
    let r = sql(&s, "SELECT * FROM v_asc").expect("a visao ASC tinha de responder");
    assert_eq!(r.inteiro_ou("devolvidas", -1), 3);
}

/// **A PROVA REAL do item: o `FROM v` le a visao, e o teste mede QUANTAS
/// linhas e QUAIS -- nao se a op respondeu.**
///
/// Sabotagem: com o `if let Some(ped) = self.selecao_sobre_visao(...)`
/// fora do `op_sql`, o `SELECT * FROM v_blumenau` volta a procurar uma
/// TABELA chamada `v_blumenau` e recusa -- e este teste reprova na
/// primeira assercao.
#[test]
fn o_from_de_uma_visao_le_a_visao() {
    let d = dir("from");
    let (s, _) = servidor(&d, Cadastro::default());
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_blumenau",
                "sql":"SELECT * FROM clientes"}"#,
    )
    .unwrap();

    let r = sql(&s, "SELECT * FROM v_blumenau").expect("a visao nao respondeu");
    assert_eq!(r.texto_ou("op", ""), "consultar");
    assert_eq!(r.inteiro_ou("devolvidas", -1), 3);

    // E o WHERE de FORA vale sobre a visao.
    let r = sql(&s, "SELECT * FROM v_blumenau WHERE cidade = 'Blumenau'").unwrap();
    let l = linhas_do_sql(&r);
    assert_eq!(l.len(), 2, "o WHERE de fora nao filtrou");
    assert_eq!(l[0].texto_ou("nome", ""), "ana");

    // A projecao e a ordem de fora tambem.
    let r = sql(
        &s,
        "SELECT nome AS quem FROM v_blumenau ORDER BY id DESC LIMIT 2",
    )
    .unwrap();
    let l = linhas_do_sql(&r);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].texto_ou("quem", ""), "caio");
    assert!(l[0].campo("cidade").is_none(), "a projecao nao cortou");

    let _ = std::fs::remove_dir_all(&d);
}

/// **A PROVA REAL do 241 (A7): a visao que PROJETA colunas nao vaza as
/// demais sob `SELECT *`.** A visao expoe so `nome`; um `SELECT * FROM v`
/// tem de devolver SO `nome`, e nao a linha inteira da tabela.
///
/// Sabotagem: passar `plano.pedido` cru como `de` em
/// `selecao_sobre_visao`, sem envolve-lo no `consultar` que aplica
/// `plano.saida`, faz o `varrer` de dentro devolver a linha inteira, e
/// `id`/`cidade` reaparecem. Este teste reprova na assercao de que `cidade`
/// sumiu -- que e o proposito de uma visao que projeta.
#[test]
fn a_visao_que_projeta_nao_vaza_as_outras_colunas() {
    let d = dir("projecao");
    let (s, _) = servidor(&d, Cadastro::default());
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_so_nome",
                "sql":"SELECT nome FROM clientes"}"#,
    )
    .unwrap();

    let r = sql(&s, "SELECT * FROM v_so_nome").expect("a visao nao respondeu");
    assert_eq!(r.texto_ou("op", ""), "consultar");
    let l = linhas_do_sql(&r);
    assert_eq!(l.len(), 3, "a visao mudou quantas linhas");
    for linha in &l {
        assert!(linha.campo("nome").is_some(), "a coluna projetada sumiu");
        assert!(
            linha.campo("cidade").is_none(),
            "a visao vazou `cidade`, que ela nao projetou: {}",
            linha.escrever()
        );
        assert!(
            linha.campo("id").is_none(),
            "a visao vazou `id`, que ela nao projetou: {}",
            linha.escrever()
        );
    }

    // E a projecao com apelido tambem: `SELECT ... AS quem` continua
    // valendo por cima da visao que ja projeta.
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_apelido",
                "sql":"SELECT nome AS pessoa FROM clientes"}"#,
    )
    .unwrap();
    let r = sql(&s, "SELECT * FROM v_apelido").expect("a visao com apelido nao respondeu");
    let l = linhas_do_sql(&r);
    assert_eq!(l.len(), 3);
    assert!(
        l[0].campo("pessoa").is_some(),
        "o apelido da visao nao chegou: {}",
        l[0].escrever()
    );
    assert!(l[0].campo("nome").is_none(), "vazou o nome sem apelido");

    let _ = std::fs::remove_dir_all(&d);
}

/// **Visao que aponta para tabela que sumiu RECUSA na hora de usar,
/// nomeando as duas.**
///
/// Nomear so a tabela mandaria quem consulta `v_x` procurar quem apagou
/// uma tabela que ele nem citou.
#[test]
fn visao_com_tabela_que_sumiu_recusa_nomeando() {
    let d = dir("sumiu");
    let (s, _) = servidor(&d, Cadastro::default());
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_folha","sql":"SELECT * FROM folha"}"#,
    )
    .unwrap();
    // A visao funciona enquanto a tabela existe -- o controle.
    assert_eq!(
        sql(&s, "SELECT * FROM v_folha")
            .unwrap()
            .inteiro_ou("devolvidas", -1),
        1
    );
    dono(
        &s,
        "excluir_tabela",
        r#"{"database":"b","tabela":"folha","confirmar":"folha"}"#,
    )
    .unwrap();
    let e = sql(&s, "SELECT * FROM v_folha").expect_err("a visao orfa respondeu");
    let t = e.to_string();
    assert!(t.contains("v_folha"), "a recusa nao diz a visao: {t}");
    assert!(t.contains("folha"), "a recusa nao diz a tabela: {t}");
    let _ = std::fs::remove_dir_all(&d);
}

/// As duas recusas da DECLARACAO: SQL que nao analisa, e nome de tabela.
#[test]
fn a_declaracao_recusa_o_que_nao_analisa_e_o_nome_de_tabela() {
    let d = dir("declara");
    let (s, _) = servidor(&d, Cadastro::default());

    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v","sql":"SELECT FROM"}"#,
    )
    .unwrap_err();
    assert!(!e.to_string().is_empty(), "recusa muda nao ensina nada");

    // Um INSERT nao e visao.
    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v","sql":"INSERT INTO clientes (id) VALUES (9)"}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("INSERT"), "{e}");

    // Nome que ja e tabela: uma das duas ficaria invisivel no FROM.
    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"clientes","sql":"SELECT * FROM folha"}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("TABELA"), "{e}");
    assert!(e.to_string().contains("clientes"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A PROVA REAL do 244 (A12): a recusa de uma visao com JOIN e HONESTA,
/// e nao se contradiz.** Um SELECT composto (JOIN, subconsulta) analisa
/// como `Comando::Consulta`, cujo `verbo()` tambem e "SELECT" -- por isso
/// a recusa antiga dizia "uma visao guarda um SELECT, e este texto e um
/// SELECT". A visao nao nasce, mas quem a pede tem de saber por que.
///
/// Sabotagem: tirar o braco `Comando::Consulta` do match faz o composto
/// cair no `outro => verbo()` e a mensagem volta a dizer "e este texto e um
/// SELECT" -- e a assercao de que a recusa NAO contem essa frase reprova.
#[test]
fn a_visao_com_join_recusa_com_mensagem_honesta() {
    let d = dir("join");
    let (s, _) = servidor(&d, Cadastro::default());

    // JOIN: composto.
    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_join",
                "sql":"SELECT c.nome FROM clientes c JOIN folha f ON c.id = f.id"}"#,
    )
    .expect_err("a visao com JOIN nasceu");
    let t = e.to_string();
    assert!(
        !t.contains("e este texto e um SELECT"),
        "a recusa ainda se contradiz: {t}"
    );
    assert!(
        t.contains("JOIN") || t.contains("substrato"),
        "a recusa nao diz por que: {t}"
    );

    // Subconsulta (`IN (SELECT ...)`): tambem composto, mesma recusa.
    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_sub",
                "sql":"SELECT id FROM clientes WHERE id IN (SELECT id FROM folha)"}"#,
    )
    .expect_err("a visao com subconsulta nasceu");
    assert!(
        !e.to_string().contains("e este texto e um SELECT"),
        "a recusa da subconsulta se contradiz: {e}"
    );

    // E a visao com JOIN NAO ficou gravada: quem falha cedo nao deixa
    // rastro que so quebra no primeiro FROM.
    let r = dono(&s, "visoes", r#"{"database":"b"}"#).unwrap();
    assert!(
        r.campo("visoes").and_then(Json::lista).unwrap().is_empty(),
        "a visao recusada ficou no catalogo"
    );

    // O controle: um INSERT continua com a recusa de sempre (nao-SELECT),
    // que nomeia o verbo de verdade -- o braco novo nao a engoliu.
    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_ins",
                "sql":"INSERT INTO clientes (id) VALUES (9)"}"#,
    )
    .expect_err("o INSERT virou visao");
    assert!(e.to_string().contains("INSERT"), "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// Listar, substituir e excluir -- e o `excluir` de quem nao existia nao
/// e erro, como todo `DROP VIEW IF EXISTS`.
#[test]
fn listar_substituir_e_excluir() {
    let d = dir("ciclo");
    let (s, _) = servidor(&d, Cadastro::default());
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v","sql":"SELECT * FROM clientes"}"#,
    )
    .unwrap();
    let r = dono(&s, "visoes", r#"{"database":"b"}"#).unwrap();
    let l = r.campo("visoes").and_then(Json::lista).unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].texto_ou("sql", ""), "SELECT * FROM clientes");

    // Sem `substituir`, o nome repetido recusa.
    let e = dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v","sql":"SELECT * FROM folha"}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("substituir"), "{e}");

    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v","sql":"SELECT * FROM folha","substituir":true}"#,
    )
    .unwrap();
    assert_eq!(
        sql(&s, "SELECT * FROM v")
            .unwrap()
            .inteiro_ou("devolvidas", -1),
        1
    );

    let r = dono(&s, "excluir_visao", r#"{"database":"b","nome":"v"}"#).unwrap();
    assert!(r.booleano_ou("excluida", false));
    let r = dono(&s, "excluir_visao", r#"{"database":"b","nome":"v"}"#).unwrap();
    assert!(
        !r.booleano_ou("excluida", true),
        "excluiu o que nao existia"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **A visao NAO e a porta dos fundos: ela le pela tabela de dentro, e a
/// tabela de dentro passa pelo portao de quem consulta -- e nao pelo de
/// quem criou.**
///
/// Este e o furo que uma visao abriria se o plano de dentro fosse
/// executado sem `executar_derivado`: o administrador cria
/// `v_folha AS SELECT * FROM folha`, e quem nao le a folha passa a ler.
#[test]
fn a_visao_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
    let d = dir("porta");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&d, cadastro);
    // O DONO cria as duas visoes.
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_folha","sql":"SELECT * FROM folha"}"#,
    )
    .unwrap();
    dono(
        &s,
        "criar_visao",
        r#"{"database":"b","nome":"v_clientes","sql":"SELECT * FROM clientes"}"#,
    )
    .unwrap();

    let pede = |corpo: &str| -> Result<Json> {
        let mut sessao = Sessao {
            usuario: ses.usuario.clone(),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(
                r#"{{"token":"t","op":"sql","database":"b","texto":{}}}"#,
                Json::texto_de(corpo).escrever()
            ),
            &mut sessao,
            "127.0.0.1",
        );
        r
    };

    // O controle: a visao sobre a tabela permitida responde.
    let ok = pede("SELECT * FROM v_clientes").expect("a visao permitida foi barrada");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 3);

    // E a visao sobre a tabela negada recusa, dizendo as duas.
    let e = pede("SELECT * FROM v_folha").expect_err("a visao leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// **Pedido 359.** A op `visoes` pede so `ler`, e devolvia o SQL da visao
/// verbatim: quem tinha a coluna `nome` negada lia o literal do `WHERE`
/// -- e o comentario -- que o dono escreveu.
///
/// # O vermelho
///
/// Com `x.para_json()` para todos, a resposta da ana trazia
/// `WHERE nome = 'caio' -- senha: hunter2`. O teste mede o TEXTO da
/// resposta, nao um veredito: o literal e o comentario ausentes, e a FORMA
/// presente (sem ela, «redigir» poderia ser apagar tudo).
#[test]
fn visoes_nao_entrega_literal_a_quem_tem_a_coluna_negada() {
    const LITERAL: &str = "caio";
    const COMENTARIO: &str = "hunter2";
    let d = dir("literal-359");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[
                {"login":"ana","id":9,"senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"clientes":{"ler":true,
                     "colunas":{"nome":{"ler":false}}}}}}},
                {"login":"rui","id":10,"senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"criar":true}}},
                {"login":"adm","id":11,"senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"b":{"ler":true,"administrar":true}}}]}"#,
    ))
    .unwrap();
    let (s, _) = servidor(&d, cadastro.clone());
    let sql_do_dono = "SELECT id, cidade FROM clientes WHERE nome = 'caio' \
                           -- senha: hunter2\n";
    // Sem login e o dono do servidor, pelo mesmo `executar` que os outros
    // testes do modulo usam; com login, o caminho inteiro do soquete.
    let pede = |login: Option<&str>, op: &str, resto: &str| -> Result<Json> {
        if login.is_none() {
            return dono(&s, op, &format!(r#"{{"database":"b"{resto}}}"#));
        }
        let mut sessao = Sessao {
            usuario: login.and_then(|l| cadastro.por_login(l).cloned()),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t","op":"{op}","database":"b"{resto}}}"#),
            &mut sessao,
            "127.0.0.1",
        );
        r
    };
    // O dono do servidor (token, sem usuario) cria; o rui cria a dele.
    pede(
        None,
        "criar_visao",
        &format!(
            r#","nome":"v_um","sql":{}"#,
            Json::texto_de(sql_do_dono).escrever()
        ),
    )
    .unwrap();
    pede(
        Some("rui"),
        "criar_visao",
        r#","nome":"v_rui","sql":"SELECT id FROM clientes WHERE cidade = 'Itajai'""#,
    )
    .unwrap();
    let sql_de = |login: Option<&str>, visao: &str| -> String {
        let r = pede(login, "visoes", "").unwrap();
        r.campo("visoes")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .find(|v| v.texto_ou("nome", "") == visao)
            .map(|v| v.texto_ou("sql", "").to_string())
            .unwrap()
    };

    // A ana, so `ler` e a coluna negada: a forma sim, o literal nao.
    let da_ana = pede(Some("ana"), "visoes", "").unwrap().escrever();
    assert!(!da_ana.contains(LITERAL), "o literal vazou: {da_ana}");
    assert!(!da_ana.contains(COMENTARIO), "o comentario vazou: {da_ana}");
    assert!(
        !da_ana.contains("Itajai"),
        "o literal da outra vazou: {da_ana}"
    );
    let forma = sql_de(Some("ana"), "v_um");
    assert!(
        forma.contains("clientes") && forma.contains("nome") && forma.contains('?'),
        "a redacao apagou a forma: {forma}"
    );

    // O comportamento VELHO onde ele vale: o dono, quem administra o
    // database e o autor continuam vendo o texto inteiro.
    assert_eq!(sql_de(None, "v_um"), sql_do_dono.trim());
    assert_eq!(sql_de(Some("adm"), "v_um"), sql_do_dono.trim());
    assert!(sql_de(Some("rui"), "v_rui").contains("'Itajai'"));
    // E o autor de UMA nao ve a de outro inteira.
    assert!(!sql_de(Some("rui"), "v_um").contains(LITERAL));

    let _ = std::fs::remove_dir_all(&d);
}

/// **Sem visao nenhuma, NADA muda no caminho do `sql`.**
///
/// O portao e um `load` atomico antes de qualquer trabalho, e a prova de
/// que ele nao mexeu em nada e esta: o `SELECT` de sempre continua virando
/// `varrer`, e nao `consultar`.
#[test]
fn sem_visao_nenhuma_o_sql_continua_como_era() {
    let d = dir("nada-muda");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = sql(&s, "SELECT * FROM clientes").unwrap();
    assert_eq!(r.texto_ou("op", ""), "varrer", "o caminho de sempre mudou");
    // E uma tabela que nao existe continua recusando como tabela, e nao
    // como visao.
    let e = sql(&s, "SELECT * FROM inventada").unwrap_err();
    assert!(e.to_string().contains("inventada"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A visao sobrevive ao restart: ela mora no diretorio do banco, e viaja
/// com o backup dele.
#[test]
fn a_visao_sobrevive_ao_restart() {
    let d = dir("restart");
    {
        let (s, _) = servidor(&d, Cadastro::default());
        dono(
            &s,
            "criar_visao",
            r#"{"database":"b","nome":"v","sql":"SELECT * FROM clientes"}"#,
        )
        .unwrap();
    }
    assert!(d.join("b").join(crate::visoes::ARQUIVO).is_file());
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s2 = Servidor::novo(c).unwrap();
    assert_eq!(
        sql(&s2, "SELECT * FROM v")
            .unwrap()
            .inteiro_ou("devolvidas", -1),
        3
    );
    let _ = std::fs::remove_dir_all(&d);
}
