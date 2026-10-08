//! **Gatilhos e procedimentos** (pedidos 49 e 50), pelo caminho de verdade:
//! o CREATE entra pela op `sql`, o disparo acontece nas operacoes de escrita
//! e o corpo fala com o motor pelos MESMOS portoes de qualquer pedido.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("gat-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn config_de(dir: &std::path::Path) -> Config {
    Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    }
}

/// Base `b` com `clientes(id, nome, cidade)` e `auditoria(evento, quem)`.
fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let s = Servidor::novo(config_de(dir)).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"cidade","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"auditoria",
                    "colunas":[{"nome":"evento","tipo":"Str(60)"},
                               {"nome":"quem","tipo":"Str(20)"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade.
fn sql(s: &Arc<Servidor>, texto: &str) -> Result<Json> {
    let mut ses = Sessao::default();
    let corpo = Json::objeto(vec![
        ("token", Json::texto_de("t")),
        ("op", Json::texto_de("sql")),
        ("database", Json::texto_de("b")),
        ("texto", Json::texto_de(texto)),
    ])
    .escrever();
    let (_, _, r) = s.despachar(&corpo, &mut ses, "127.0.0.1");
    r
}

fn inserir(s: &Arc<Servidor>, tabela: &str, linha: &str) -> Result<Json> {
    s.executar(
        "inserir",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"{tabela}","linha":{linha}}}"#
        )),
        &Sessao::default(),
    )
}

/// Quantas linhas quem consulta VE — o excluir suave tira da lista sem
/// mexer no contador do cabecalho, entao contar `registros` mentiria.
fn registros(s: &Arc<Servidor>, tabela: &str) -> i64 {
    s.executar(
        "varrer",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"{tabela}","max":100}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .map(|l| l.len() as i64)
    .unwrap_or(-1)
}

/// O caso 1 do pedido 49: BEFORE INSERT normaliza um campo.
#[test]
fn before_insert_normaliza_o_campo() {
    let guarda = dir_temp("normaliza");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW \
             SET NEW.cidade = UPPER(TRIM(NEW.cidade))",
    )
    .unwrap();
    let r = inserir(
        &s,
        "clientes",
        r#"{"id":1,"nome":"Ana","cidade":"  blumenau "}"#,
    )
    .unwrap();
    let rowid = r.inteiro_ou("rowid", 0);
    let linha = s
        .executar(
            "ler",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"clientes","rowid":{rowid}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(linha.texto_ou("cidade", ""), "BLUMENAU");
    // E a resposta nao ganhou campo novo: sem aviso, a forma e a velha.
    assert!(r.campo("gatilhos_avisos").is_none());
}

/// O caso 2: SIGNAL cancela — o erro leva a MESSAGE_TEXT e a linha NAO
/// entra. E o mesmo gatilho deixa passar a linha que obedece a regra.
#[test]
fn sinal_cancela_a_escrita_e_a_linha_nao_entra() {
    let guarda = dir_temp("sinal");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER exige_nome BEFORE INSERT ON clientes FOR EACH ROW \
             IF NEW.nome IS NULL OR NEW.nome = '' THEN \
               SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cliente sem nome nao entra'; \
             END IF",
    )
    .unwrap();
    let e = inserir(&s, "clientes", r#"{"id":1,"nome":"","cidade":"X"}"#).unwrap_err();
    assert!(
        matches!(&e, PhxError::Sinal { estado, .. } if estado == "45000"),
        "esperava Sinal, veio {e}"
    );
    assert!(e.to_string().contains("cliente sem nome nao entra"), "{e}");
    assert_eq!(e.codigo(), 3005);
    assert_eq!(registros(&s, "clientes"), 0, "a linha recusada ENTROU");

    inserir(&s, "clientes", r#"{"id":1,"nome":"Ana","cidade":"X"}"#).unwrap();
    assert_eq!(registros(&s, "clientes"), 1);
}

/// O caso 3: AFTER INSERT grava a auditoria noutra tabela, com o NEW
/// como a linha FICOU gravada.
#[test]
fn after_insert_audita_noutra_tabela() {
    let guarda = dir_temp("audita");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW \
             INSERT INTO auditoria (evento, quem) \
             VALUES (CONCAT('entrou ', NEW.nome), 'gatilho')",
    )
    .unwrap();
    let r = inserir(&s, "clientes", r#"{"id":7,"nome":"Maria","cidade":"J"}"#).unwrap();
    assert!(r.campo("gatilhos_avisos").is_none(), "{}", r.escrever());
    let audit = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"auditoria","max":10}"#),
            &Sessao::default(),
        )
        .unwrap();
    let linhas = audit.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas.len(), 1);
    assert_eq!(linhas[0].texto_ou("evento", ""), "entrou Maria");
}

/// UPDATE ve OLD e NEW; DELETE ve OLD e o SIGNAL protege a linha — nos
/// dois modos de excluir, porque nos dois a linha some da lista.
#[test]
fn update_e_delete_veem_old() {
    let guarda = dir_temp("old");
    let s = servidor(&guarda);
    inserir(
        &s,
        "clientes",
        r#"{"id":1,"nome":"protegido","cidade":"X"}"#,
    )
    .unwrap();
    inserir(&s, "clientes", r#"{"id":2,"nome":"comum","cidade":"X"}"#).unwrap();
    sql(
        &s,
        "CREATE TRIGGER sem_renomear BEFORE UPDATE ON clientes FOR EACH ROW \
             IF NEW.nome <> OLD.nome THEN \
               SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'nome nao se troca'; \
             END IF",
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER sem_excluir BEFORE DELETE ON clientes FOR EACH ROW \
             IF OLD.nome = 'protegido' THEN \
               SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'este nao sai'; \
             END IF",
    )
    .unwrap();

    // Trocar a cidade pode; trocar o nome, nao.
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"b","tabela":"clientes","rowid":1,
                    "linha":{"id":1,"nome":"protegido","cidade":"Y"}}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    let e = s
        .executar(
            "atualizar",
            &pedido(
                r#"{"database":"b","tabela":"clientes","rowid":1,
                        "linha":{"id":1,"nome":"outro","cidade":"Y"}}"#,
            ),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("nome nao se troca"), "{e}");

    // Excluir o protegido nao pode — nem suave, nem fisico.
    for extra in ["", r#","fisico":true"#] {
        let e = s
            .executar(
                "excluir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"clientes","rowid":1{extra}}}"#
                )),
                &Sessao::default(),
            )
            .unwrap_err();
        assert!(e.to_string().contains("este nao sai"), "{e}");
    }
    assert_eq!(registros(&s, "clientes"), 2);
    // O comum sai.
    s.executar(
        "excluir",
        &pedido(r#"{"database":"b","tabela":"clientes","rowid":2}"#),
        &Sessao::default(),
    )
    .unwrap();
    assert_eq!(registros(&s, "clientes"), 1);
}

/// O caso 4 (pedido 50): procedimento com IN, OUT e WHILE somando.
#[test]
fn procedimento_com_in_out_e_while() {
    let guarda = dir_temp("proc");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE PROCEDURE somar(IN ate INT, OUT total INT) BEGIN \
               DECLARE i INT DEFAULT 1; \
               SET total = 0; \
               WHILE i <= ate DO \
                 SET total = total + i; \
                 SET i = i + 1; \
               END WHILE; \
             END",
    )
    .unwrap();
    let r = sql(&s, "CALL somar(100)").unwrap();
    assert_eq!(r.texto_ou("procedimento", ""), "somar");
    assert_eq!(r.campo("saida").unwrap().inteiro_ou("total", -1), 5050);
}

/// Pedido 238, sonda viva: o `{call p(?, ?)}` do ODBC chega como `CALL
/// p(?, ?)` com `parametros` (o `OUT` puro manda NULL). O driver ligava o
/// `OUT` e o servidor nunca resolvia o `?` do `CALL` -- o erro era
/// «esperava um valor e veio ?». Agora o `OUT` volta em `saida`.
#[test]
fn call_com_interrogacao_do_odbc_devolve_a_saida() {
    let guarda = dir_temp("call-odbc");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE PROCEDURE dobro(IN x INT, OUT y INT) SET y = x * 2",
    )
    .unwrap();
    let mut ses = Sessao::default();
    let corpo = Json::objeto(vec![
        ("token", Json::texto_de("t")),
        ("op", Json::texto_de("sql")),
        ("database", Json::texto_de("b")),
        ("texto", Json::texto_de("CALL dobro(?, ?)")),
        (
            "parametros",
            Json::Lista(vec![Json::de_u64(21), Json::Nulo]),
        ),
    ])
    .escrever();
    let (_, _, r) = s.despachar(&corpo, &mut ses, "127.0.0.1");
    let r = r.unwrap();
    assert_eq!(r.campo("saida").unwrap().inteiro_ou("y", -1), 42);
    // O comportamento velho: sem `parametros` o `?` segue recusado.
    assert!(sql(&s, "CALL dobro(?, ?)").is_err());
}

/// Procedimento le o motor: SELECT … INTO com COUNT(*) e com coluna.
#[test]
fn procedimento_le_com_select_into() {
    let guarda = dir_temp("into");
    let s = servidor(&guarda);
    inserir(&s, "clientes", r#"{"id":1,"nome":"Ana","cidade":"BNU"}"#).unwrap();
    inserir(&s, "clientes", r#"{"id":2,"nome":"Bia","cidade":"JLE"}"#).unwrap();
    sql(
        &s,
        "CREATE PROCEDURE resumo(IN qual INT, OUT quantos INT, OUT nome_dele VARCHAR(20)) \
             BEGIN \
               SELECT COUNT(*) INTO quantos FROM clientes; \
               SELECT nome INTO nome_dele FROM clientes WHERE id = qual; \
             END",
    )
    .unwrap();
    let r = sql(&s, "CALL resumo(2)").unwrap();
    let saida = r.campo("saida").unwrap();
    assert_eq!(saida.inteiro_ou("quantos", -1), 2);
    assert_eq!(saida.texto_ou("nome_dele", ""), "Bia");
}

/// **A porta dos fundos continua fechada.** CALL roda com o poder de quem
/// chama: quem nao pode inserir na tabela nao passa a poder porque um
/// administrador guardou um INSERT nela dentro de um procedimento.
///
/// A prova real deste teste esta em docs/TRIGGERS.md: com o
/// `executar_derivado` do motor trocado por `executar` (sem portao), ele
/// FALHA com a linha gravada — que e o furo que ele existe para impedir.
#[test]
fn call_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
    let dir = dir_temp("porta");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"b":{"ler":true,"inserir":true,
                               "tabelas":{"auditoria":{"ler":true}}}}}]}"#,
    ))
    .unwrap();
    let mut c = config_de(&dir);
    c.cadastro = cadastro.clone();
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"auditoria",
                    "colunas":[{"nome":"evento","tipo":"Str(60)"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    // O dono guarda o procedimento que grava na tabela fechada.
    s.op_sql(
        &pedido(
            r#"{"database":"b",
                    "texto":"CREATE PROCEDURE fura() INSERT INTO auditoria (evento) VALUES ('furou')"}"#,
        ),
        &dono,
    )
    .unwrap();

    // A ana pode ler a base e ate inserir nas outras tabelas — mas na
    // auditoria so pode ler, e o CALL nao muda isso.
    let ana = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let e = s
        .op_sql(&pedido(r#"{"database":"b","texto":"CALL fura()"}"#), &ana)
        .unwrap_err();
    assert!(
        e.to_string().contains("permissao") || e.to_string().contains("acesso"),
        "{e}"
    );
    let quantos = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"auditoria","max":10}"#),
            &dono,
        )
        .unwrap()
        .inteiro_ou("registros", -1);
    assert_eq!(quantos, 0, "o CALL furou o portao e gravou");
}

/// Criar, excluir e listar rotina exigem administrar — inserir na tabela
/// nao basta, pela escalada descrita no comentario do `executar_rotina`.
#[test]
fn criar_rotina_exige_administrar() {
    let dir = dir_temp("administrar");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"b":{"ler":true,"inserir":true,"criar":true}}}]}"#,
    ))
    .unwrap();
    let mut c = config_de(&dir);
    c.cadastro = cadastro.clone();
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    let ana = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    for texto in [
        "CREATE TRIGGER t BEFORE INSERT ON c FOR EACH ROW SET NEW.id = 1",
        "CREATE PROCEDURE p() SET x = 1",
        "DROP TRIGGER t",
        "SHOW TRIGGERS",
        "SHOW PROCEDURES",
    ] {
        let e = s
            .op_sql(
                &pedido(&format!(r#"{{"database":"b","texto":"{texto}"}}"#)),
                &ana,
            )
            .unwrap_err();
        assert!(
            e.to_string().contains("administrar"),
            "{texto} passou sem administrar: {e}"
        );
    }
}

/// **O comportamento velho.** Tabela sem gatilho grava exatamente como
/// antes — inclusive quando OUTRA tabela tem gatilho, que e quando o
/// portao atomico esta ligado e a consulta ao registro acontece.
#[test]
fn sem_gatilho_nada_muda() {
    let guarda = dir_temp("velho");
    let s = servidor(&guarda);
    // Gatilho na auditoria, nunca na clientes.
    sql(
        &s,
        "CREATE TRIGGER na_outra BEFORE INSERT ON auditoria FOR EACH ROW \
             SET NEW.quem = 'x'",
    )
    .unwrap();
    let r = inserir(&s, "clientes", r#"{"id":1,"nome":"ana","cidade":"bnu"}"#).unwrap();
    assert_eq!(r.chaves(), vec!["rowid", "registros"], "a forma mudou");
    let linha = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"clientes","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    // Minusculo como entrou: nenhum gatilho alheio encostou aqui.
    assert_eq!(linha.texto_ou("cidade", ""), "bnu");
    // Atualizar e excluir tambem continuam com a forma velha.
    let r = s
        .executar(
            "atualizar",
            &pedido(
                r#"{"database":"b","tabela":"clientes","rowid":1,
                        "linha":{"id":1,"nome":"ana","cidade":"jle"}}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.chaves(), vec!["rowid", "versao"]);
    let r = s
        .executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"clientes","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(
        r.chaves(),
        vec!["rowid", "excluido", "modo", "na_lixeira", "reversivel"]
    );
}

/// O gatilho sobrevive ao reinicio: sai do `gatilhos.json` e volta
/// compilado.
#[test]
fn o_gatilho_sobrevive_ao_reinicio() {
    let dir = dir_temp("reinicio");
    {
        let s = servidor(&dir);
        sql(
            &s,
            "CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW \
                 SET NEW.cidade = UPPER(NEW.cidade)",
        )
        .unwrap();
    }
    // Outro processo, mesma base.
    let s = Servidor::novo(config_de(&dir)).unwrap();
    inserir(&s, "clientes", r#"{"id":1,"nome":"Ana","cidade":"bnu"}"#).unwrap();
    let linha = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"clientes","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(linha.texto_ou("cidade", ""), "BNU");
}

#[test]
fn drop_tira_e_show_lista() {
    let guarda = dir_temp("drop");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW \
             SET NEW.cidade = UPPER(NEW.cidade)",
    )
    .unwrap();
    sql(&s, "CREATE PROCEDURE nada() SET @x = 1").unwrap_err(); // @ nao existe
    sql(
        &s,
        "CREATE PROCEDURE dobro(IN x INT, OUT y INT) SET y = x * 2",
    )
    .unwrap();

    let r = sql(&s, "SHOW TRIGGERS").unwrap();
    assert_eq!(r.inteiro_ou("total", -1), 1);
    let g = &r.campo("gatilhos").and_then(Json::lista).unwrap()[0];
    assert_eq!(g.texto_ou("nome", ""), "normaliza");
    assert_eq!(g.texto_ou("quando", ""), "BEFORE");
    assert!(
        g.texto_ou("corpo", "").contains("UPPER"),
        "o corpo nao voltou"
    );

    let r = sql(&s, "SHOW PROCEDURES").unwrap();
    assert_eq!(r.inteiro_ou("total", -1), 1);

    sql(&s, "DROP TRIGGER normaliza").unwrap();
    sql(&s, "DROP PROCEDURE dobro").unwrap();
    assert_eq!(sql(&s, "SHOW TRIGGERS").unwrap().inteiro_ou("total", -1), 0);
    // Depois do DROP, a escrita volta a ser a crua.
    inserir(&s, "clientes", r#"{"id":1,"nome":"Ana","cidade":"bnu"}"#).unwrap();
    let linha = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"clientes","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(linha.texto_ou("cidade", ""), "bnu");
    // E o segundo DROP recusa — mas com IF EXISTS, passa.
    assert!(sql(&s, "DROP TRIGGER normaliza").is_err());
    assert!(sql(&s, "DROP TRIGGER IF EXISTS normaliza").is_ok());
}

/// O lote passa pelo BEFORE linha a linha: a recusada vira erro DO LOTE,
/// com a posicao, e as outras entram — o contrato de sempre do lote.
#[test]
fn lote_passa_pelo_before_por_linha() {
    let guarda = dir_temp("lote");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER exige_nome BEFORE INSERT ON clientes FOR EACH ROW \
             BEGIN \
               IF NEW.nome = '' THEN \
                 SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'sem nome'; \
               END IF; \
               SET NEW.cidade = UPPER(NEW.cidade); \
             END",
    )
    .unwrap();
    let r = s
        .executar(
            "inserir_lote",
            &pedido(
                r#"{"database":"b","tabela":"clientes","parar_no_erro":false,
                        "linhas":[{"id":1,"nome":"Ana","cidade":"bnu"},
                                  {"id":2,"nome":"","cidade":"x"},
                                  {"id":3,"nome":"Bia","cidade":"jle"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("gravadas", -1), 2);
    assert_eq!(r.inteiro_ou("recusadas", -1), 1);
    let erros = r.campo("erros").and_then(Json::lista).unwrap();
    assert_eq!(erros[0].inteiro_ou("linha", -1), 2);
    assert!(erros[0].texto_ou("erro", "").contains("sem nome"));
    assert_eq!(registros(&s, "clientes"), 2);
    // E a normalizacao valeu para as que entraram.
    let l = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"clientes","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(l.texto_ou("cidade", ""), "BNU");
}

/// Falha de AFTER nao desfaz a escrita — nao ha transacao — e vira o
/// aviso `gatilhos_avisos`, com o nome do gatilho, numa resposta `ok`.
#[test]
fn falha_de_after_vira_aviso_e_a_escrita_fica() {
    let guarda = dir_temp("aviso");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW \
             INSERT INTO tabela_que_nao_existe (a) VALUES (1)",
    )
    .unwrap();
    let r = inserir(&s, "clientes", r#"{"id":1,"nome":"Ana","cidade":"X"}"#).unwrap();
    assert_eq!(registros(&s, "clientes"), 1, "a escrita tinha de ficar");
    let avisos = r.campo("gatilhos_avisos").and_then(Json::lista).unwrap();
    assert!(
        avisos[0].texto().unwrap_or("").contains("audita"),
        "{}",
        r.escrever()
    );
}

/// Excluir a tabela leva os gatilhos dela: o orfao dispararia contra uma
/// homonima futura.
#[test]
fn excluir_tabela_leva_os_gatilhos() {
    let guarda = dir_temp("orfao");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW \
             SET NEW.cidade = UPPER(NEW.cidade)",
    )
    .unwrap();
    let r = s
        .executar(
            "excluir_tabela",
            &pedido(r#"{"database":"b","tabela":"clientes","confirmar":"clientes"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("gatilhos_apagados", -1), 1);
    assert_eq!(sql(&s, "SHOW TRIGGERS").unwrap().inteiro_ou("total", -1), 0);
    // Nota de rodape achada por ESTE teste e fora do escopo dele:
    // `excluir_tabela` nao apaga o `.trash`, entao recriar a homonima
    // recusa hoje por outro motivo. O orfao esta provado pelo registro.
}

/// Coluna de sistema nao se toca por gatilho: a ordem de digitacao e
/// sagrada, e um `SET NEW.rownum` seria o jeito novo de quebra-la.
#[test]
fn coluna_de_sistema_recusa_o_set() {
    let guarda = dir_temp("sistema");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER esperto BEFORE INSERT ON clientes FOR EACH ROW \
             SET NEW.rownum = 1",
    )
    .unwrap();
    let e = inserir(&s, "clientes", r#"{"id":1,"nome":"a","cidade":"b"}"#).unwrap_err();
    assert!(e.to_string().contains("sistema"), "{e}");
    assert_eq!(registros(&s, "clientes"), 0);
}

/// Servidor somente-leitura nao cria nem exclui rotina — os arquivos de
/// rotina sao escrita como qualquer outra.
#[test]
fn somente_leitura_recusa_criar_rotina() {
    let dir = dir_temp("soler");
    {
        let s = servidor(&dir);
        drop(s);
    }
    let mut c = config_de(&dir);
    c.somente_leitura = true;
    let s = Servidor::novo(c).unwrap();
    let e = sql(
        &s,
        "CREATE TRIGGER t BEFORE INSERT ON clientes FOR EACH ROW SET NEW.cidade = 'x'",
    )
    .unwrap_err();
    assert!(e.to_string().contains("somente leitura"), "{e}");
}

// -----------------------------------------------------------------------
// Os gatilhos do UPSERT honram o ramo que ele virou (o gap da G4-MOTOR no
// fim do pedido 245)
// -----------------------------------------------------------------------

/// Le a linha inteira pelo rowid.
fn linha(s: &Arc<Servidor>, tabela: &str, rowid: u64) -> Json {
    s.executar(
        "ler",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"{tabela}","rowid":{rowid}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
}

/// Os eventos gravados na auditoria, na ordem em que entraram.
fn auditoria(s: &Arc<Servidor>) -> Vec<String> {
    s.executar(
        "varrer",
        &pedido(r#"{"database":"b","tabela":"auditoria","max":100}"#),
        &Sessao::default(),
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .map(|l| {
        l.iter()
            .map(|x| x.texto_ou("evento", "").to_string())
            .collect()
    })
    .unwrap_or_default()
}

/// O upsert em `clientes` que ATUALIZA quando a chave existe, com ou sem
/// o SET (`atualizar`).
fn upsert(s: &Arc<Servidor>, ses: &Sessao, corpo: &str) -> Result<Json> {
    s.executar(
        "inserir",
        &pedido(&format!(
            r#"{{"database":"b","tabela":"clientes","se_existir":"atualizar",{corpo}}}"#
        )),
        ses,
    )
}

/// **No upsert que ATUALIZA, o BEFORE UPDATE roda sobre a linha MESCLADA
/// -- a gravada com o SET por cima -- e nao sobre a do VALUES.** E o gap
/// que a G4-MOTOR deixou nomeado no fim do pedido 245.
///
/// O gatilho le uma coluna que o VALUES NAO traz (a cidade) e deixa o que
/// viu no nome, que e a unica saida de um BEFORE. Na linha do VALUES a
/// cidade e nula; na mesclada e "Blumenau". Um segundo gatilho RECUSA a
/// linha sem cidade: rodar o BEFORE UPDATE sobre a linha do VALUES
/// derrubaria o upsert por uma coluna que a linha final tem.
///
/// Com o defeito reposto o BEFORE UPDATE nem roda -- o unico BEFORE que
/// rodava era o de INSERT, sobre a linha crua -- e o nome fica com o "B"
/// do SET.
///
/// Lei dos tres motores, aceite automatico: PostgreSQL (`ON CONFLICT DO
/// UPDATE`), MariaDB e MySQL (`ON DUPLICATE KEY UPDATE`) disparam o
/// BEFORE INSERT sobre a linha proposta e, no ramo que atualiza, o BEFORE
/// UPDATE com NEW = a linha existente com o SET por cima e OLD = a
/// existente.
#[test]
fn no_upsert_que_atualiza_o_before_update_ve_a_linha_mesclada() {
    let guarda = dir_temp("upsert-before");
    let s = servidor(&guarda);
    inserir(
        &s,
        "clientes",
        r#"{"id":1,"nome":"Ana","cidade":"Blumenau"}"#,
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER exige_cidade BEFORE UPDATE ON clientes FOR EACH ROW \
             IF NEW.cidade IS NULL THEN \
               SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cidade obrigatoria'; \
             END IF",
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER carimba BEFORE UPDATE ON clientes FOR EACH ROW \
             SET NEW.nome = CONCAT('viu ', IFNULL(NEW.cidade, 'nada'))",
    )
    .unwrap();

    let r = upsert(
        &s,
        &Sessao::default(),
        r#""linha":{"id":1,"nome":"x"},"atualizar":{"nome":"B"}"#,
    )
    .expect("o BEFORE UPDATE julgou a linha do VALUES, que nao tem cidade");
    assert!(r.booleano_ou("atualizada", false), "{}", r.escrever());
    let l = linha(&s, "clientes", 1);
    assert_eq!(l.texto_ou("cidade", ""), "Blumenau", "{}", l.escrever());
    assert_eq!(
        l.texto_ou("nome", ""),
        "viu Blumenau",
        "o BEFORE UPDATE nao viu a linha mesclada: {}",
        l.escrever()
    );
    assert_eq!(registros(&s, "clientes"), 1);
}

/// **A ordem e BEFORE INSERT primeiro, sobre a linha proposta, e depois
/// o BEFORE UPDATE sobre o que sobrou dela.** Sem o SET, a linha do
/// pedido inteira e o que entra por cima (o contrato do protocolo), entao
/// o que o BEFORE INSERT deixou nela e o que o BEFORE UPDATE ve -- e o
/// mesmo que o `EXCLUDED` do PostgreSQL carrega: os efeitos dos BEFORE
/// INSERT.
#[test]
fn o_before_insert_roda_primeiro_e_o_before_update_ve_o_que_ele_deixou() {
    let guarda = dir_temp("upsert-ordem");
    let s = servidor(&guarda);
    inserir(
        &s,
        "clientes",
        r#"{"id":1,"nome":"Ana","cidade":"Blumenau"}"#,
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER maiusc BEFORE INSERT ON clientes FOR EACH ROW \
             SET NEW.cidade = UPPER(NEW.cidade)",
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER carimba BEFORE UPDATE ON clientes FOR EACH ROW \
             SET NEW.nome = CONCAT('viu ', IFNULL(NEW.cidade, 'nada'))",
    )
    .unwrap();
    upsert(
        &s,
        &Sessao::default(),
        r#""linha":{"id":1,"nome":"x","cidade":"joinville"}"#,
    )
    .unwrap();
    let l = linha(&s, "clientes", 1);
    assert_eq!(l.texto_ou("cidade", ""), "JOINVILLE", "{}", l.escrever());
    assert_eq!(l.texto_ou("nome", ""), "viu JOINVILLE", "{}", l.escrever());
}

/// **O AFTER que dispara e o do ramo que o upsert VIROU: AFTER UPDATE
/// quando atualizou, AFTER INSERT quando inseriu -- nunca os dois, e o de
/// UPDATE ve o OLD.** Com o defeito reposto, o ramo de atualizacao
/// disparava o AFTER INSERT com a linha como ficou: uma auditoria de
/// "entrou" para uma linha que ja estava la.
#[test]
fn no_upsert_o_after_e_o_do_ramo_que_ele_virou() {
    let guarda = dir_temp("upsert-after");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER entrou AFTER INSERT ON clientes FOR EACH ROW \
             INSERT INTO auditoria (evento, quem) \
             VALUES (CONCAT('entrou ', NEW.nome), 'g')",
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER mudou AFTER UPDATE ON clientes FOR EACH ROW \
             INSERT INTO auditoria (evento, quem) \
             VALUES (CONCAT('mudou ', OLD.nome, ' para ', NEW.nome), 'g')",
    )
    .unwrap();
    let ses = Sessao::default();
    // A chave nao existe: inseriu.
    let r = upsert(&s, &ses, r#""linha":{"id":1,"nome":"Ana","cidade":"X"}"#).unwrap();
    assert!(!r.booleano_ou("atualizada", false), "{}", r.escrever());
    assert_eq!(auditoria(&s), vec!["entrou Ana"]);
    // A chave existe: atualizou -- e o AFTER UPDATE ve o OLD.
    let r = upsert(
        &s,
        &ses,
        r#""linha":{"id":1,"nome":"x"},"atualizar":{"nome":"Bia"}"#,
    )
    .unwrap();
    assert!(r.booleano_ou("atualizada", false), "{}", r.escrever());
    assert!(r.campo("gatilhos_avisos").is_none(), "{}", r.escrever());
    assert_eq!(auditoria(&s), vec!["entrou Ana", "mudou Ana para Bia"]);
}

/// **O upsert IGNORADO nao dispara AFTER nenhum: nada foi gravado.** Com
/// o defeito reposto, o AFTER INSERT rodava com a linha que JA ESTAVA LA
/// como NEW -- "entrou Ana" numa auditoria, por um pedido que nao gravou
/// byte nenhum. E o consenso dos tres: AFTER so roda para a linha que a
/// operacao de fato gravou (o `DO NOTHING` do PostgreSQL e o `INSERT
/// IGNORE` do MySQL/MariaDB pulam o AFTER INSERT da linha ignorada).
#[test]
fn o_upsert_ignorado_nao_dispara_after_nenhum() {
    let guarda = dir_temp("upsert-ignorado");
    let s = servidor(&guarda);
    sql(
        &s,
        "CREATE TRIGGER entrou AFTER INSERT ON clientes FOR EACH ROW \
             INSERT INTO auditoria (evento, quem) \
             VALUES (CONCAT('entrou ', NEW.nome), 'g')",
    )
    .unwrap();
    inserir(&s, "clientes", r#"{"id":1,"nome":"Ana","cidade":"X"}"#).unwrap();
    assert_eq!(auditoria(&s), vec!["entrou Ana"]);
    let r = s
        .executar(
            "inserir",
            &pedido(
                r#"{"database":"b","tabela":"clientes","se_existir":"ignorar",
                        "linha":{"id":1,"nome":"outra"}}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert!(r.booleano_ou("ignorada", false), "{}", r.escrever());
    assert_eq!(
        auditoria(&s),
        vec!["entrou Ana"],
        "o AFTER rodou sem gravacao"
    );
    assert_eq!(linha(&s, "clientes", 1).texto_ou("nome", ""), "Ana");
}

/// **Dentro da transacao, o mesmo: o BEFORE UPDATE do upsert que virou
/// `atualizar` roda na INSTRUCAO, sobre a mesclada.** O irmao do
/// `op_inserir` e o `empilhar`, que chama as mesmas funcoes na mesma
/// ordem; o AFTER UPDATE ja rodava no COMMIT pela acao empilhada.
#[test]
fn dentro_da_transacao_o_before_update_do_upsert_ve_a_mesclada() {
    let guarda = dir_temp("upsert-tx");
    let s = servidor(&guarda);
    inserir(
        &s,
        "clientes",
        r#"{"id":1,"nome":"Ana","cidade":"Blumenau"}"#,
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER exige_cidade BEFORE UPDATE ON clientes FOR EACH ROW \
             IF NEW.cidade IS NULL THEN \
               SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cidade obrigatoria'; \
             END IF",
    )
    .unwrap();
    sql(
        &s,
        "CREATE TRIGGER carimba BEFORE UPDATE ON clientes FOR EACH ROW \
             SET NEW.nome = CONCAT('viu ', IFNULL(NEW.cidade, 'nada'))",
    )
    .unwrap();
    let ses = Sessao {
        ligacao: 7,
        ..Sessao::default()
    };
    s.executar("begin", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    let r = upsert(
        &s,
        &ses,
        r#""linha":{"id":1,"nome":"x"},"atualizar":{"nome":"B"}"#,
    )
    .expect("o BEFORE UPDATE julgou a linha do VALUES, que nao tem cidade");
    assert_eq!(r.texto_ou("acao", ""), "atualizar", "{}", r.escrever());
    s.executar("commit", &Json::objeto(vec![]), &ses).unwrap();
    let l = linha(&s, "clientes", 1);
    assert_eq!(l.texto_ou("cidade", ""), "Blumenau", "{}", l.escrever());
    assert_eq!(
        l.texto_ou("nome", ""),
        "viu Blumenau",
        "o BEFORE UPDATE nao viu a linha mesclada: {}",
        l.escrever()
    );
}

/// **Pedido 595, contra o sistema operacional:** o cadastro por database
/// (`gatilhos.json`, `procedimentos.json`, `visoes.json`) vai ao disco
/// antes da resposta. Era `escrever_do_banco` NO LUGAR e `remove_file`,
/// sem `fsync` nenhum: uma queda devolvia o gatilho excluido -- e, no
/// `excluir_tabela`, o gatilho de uma tabela que nao existe mais, pronto
/// para disparar sobre a homonima que alguem criasse depois.
mod cadastro_vai_ao_disco_595 {
    use super::*;

    /// A marca que separa, no traco, o que cada pedido fez: um `openat`
    /// de um nome que nao existe (a mesma do 589/591).
    fn passo(base: &std::path::Path, n: u32) {
        let _ = std::fs::File::open(base.join(format!("passo-{n}")));
    }

    /// Os oito pedidos, cada um entre dois [`passo`]s.
    #[test]
    #[ignore = "roda so dentro de o_cadastro_vai_ao_disco_antes_da_resposta"]
    fn filho_do_cadastro() {
        let base = std::path::PathBuf::from(std::env::var("PHX_595_DIR").unwrap());
        let s = servidor(&base);
        let dono = Sessao::default();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"sai",
                        "colunas":[{"nome":"id","tipo":"Int4"}]}"#,
            ),
            &dono,
        )
        .unwrap();
        let corpo = "FOR EACH ROW SET NEW.id = NEW.id";
        passo(&base, 0);
        sql(
            &s,
            &format!("CREATE TRIGGER g1 BEFORE INSERT ON clientes {corpo}"),
        )
        .unwrap();
        passo(&base, 1);
        sql(
            &s,
            &format!("CREATE TRIGGER g2 BEFORE INSERT ON sai {corpo}"),
        )
        .unwrap();
        passo(&base, 2);
        sql(&s, "DROP TRIGGER g1").unwrap();
        passo(&base, 3);
        let r = s
            .executar(
                "excluir_tabela",
                &pedido(r#"{"database":"b","tabela":"sai","confirmar":"sai"}"#),
                &dono,
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("gatilhos_apagados", -1), 1);
        passo(&base, 4);
        sql(&s, "CREATE PROCEDURE p(IN x INT, OUT y INT) SET y = x").unwrap();
        passo(&base, 5);
        sql(&s, "DROP PROCEDURE p").unwrap();
        passo(&base, 6);
        s.executar(
            "criar_visao",
            &pedido(r#"{"database":"b","nome":"v","sql":"SELECT * FROM clientes"}"#),
            &dono,
        )
        .unwrap();
        passo(&base, 7);
        let r = s
            .executar(
                "excluir_visao",
                &pedido(r#"{"database":"b","nome":"v"}"#),
                &dono,
            )
            .unwrap();
        assert_eq!(r.campo("excluida"), Some(&Json::Bool(true)));
        passo(&base, 8);
    }

    /// Em cada pedido, antes de o seguinte comecar: a regravacao e
    /// `fsync` do temporario, `rename` sobre o nome e `fsync` da pasta; a
    /// exclusao e `unlink` e `fsync` da pasta. E no `excluir_tabela`, o
    /// `gatilhos.json` sai ANTES do primeiro `fsync` da pasta que leva o
    /// sumico da tabela -- ao contrario, a queda entre os dois deixava o
    /// gatilho orfao.
    ///
    /// **Nao medido:** a queda em si (pede derrubar a maquina). O que se
    /// prova e o DESCRITOR e a ORDEM das chamadas, que e o que o conserto
    /// muda.
    #[test]
    fn o_cadastro_vai_ao_disco_antes_da_resposta() {
        if std::process::Command::new("strace")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("sem strace nesta maquina: a prova do 595 NAO MEDIDA");
            return;
        }
        let t = dir_temp("595-strace");
        let base = std::fs::canonicalize(&t.0).unwrap();
        let traco = t.0.join("traco.txt");
        let saida = std::process::Command::new("strace")
            .args([
                "-f",
                "-y",
                "-e",
                "trace=openat,fsync,unlink,unlinkat,rename,renameat,renameat2",
                "-o",
            ])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "servidor::testes_gatilhos::cadastro_vai_ao_disco_595::filho_do_cadastro",
            ])
            .env("PHX_595_DIR", &base)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto.lines().filter(|l| l.contains(" = ")).collect();
        let marca = |n: u32| {
            let alvo = format!("\"{}\"", base.join(format!("passo-{n}")).display());
            linhas
                .iter()
                .position(|l| l.contains("openat(") && l.contains(&alvo))
                .unwrap_or_else(|| panic!("a premissa: o passo {n} no traco:\n{texto}"))
        };
        let ok = |l: &str| l.trim_end().ends_with("= 0");
        let pasta = base.join("b");
        let aspas = |nome: &str| format!("\"{}\"", pasta.join(nome).display());
        let fsync_de = |nome: &str| format!("<{}>)", pasta.join(nome).display());
        let fsync_da_pasta = format!("<{}>)", pasta.display());
        let mut erros = Vec::new();
        // (passo, arquivo, true = regrava, false = sai)
        let casos = [
            (1, crate::rotinas::ARQUIVO_GATILHOS, true),
            (2, crate::rotinas::ARQUIVO_GATILHOS, true),
            (3, crate::rotinas::ARQUIVO_GATILHOS, true),
            (4, crate::rotinas::ARQUIVO_GATILHOS, false),
            (5, crate::rotinas::ARQUIVO_PROCEDIMENTOS, true),
            (6, crate::rotinas::ARQUIVO_PROCEDIMENTOS, false),
            (7, crate::visoes::ARQUIVO, true),
            (8, crate::visoes::ARQUIVO, false),
        ];
        for (n, arquivo, regrava) in casos {
            let janela = &linhas[marca(n - 1) + 1..marca(n)];
            let novo = format!("{arquivo}.novo");
            let evento = janela.iter().position(|l| {
                ok(l)
                    && if regrava {
                        l.contains("rename")
                            && l.contains(&aspas(&novo))
                            && l.contains(&aspas(arquivo))
                    } else {
                        l.contains("unlink") && l.contains(&aspas(arquivo))
                    }
            });
            let Some(evento) = evento else {
                erros.push(format!(
                    "passo {n}: {arquivo} nao foi {} pela troca/unlink",
                    if regrava { "regravado" } else { "apagado" }
                ));
                continue;
            };
            if regrava
                && !janela[..evento]
                    .iter()
                    .any(|l| l.contains("fsync(") && l.contains(&fsync_de(&novo)) && ok(l))
            {
                erros.push(format!("passo {n}: o {novo} trocou de nome sem fsync"));
            }
            if !janela[evento + 1..]
                .iter()
                .any(|l| l.contains("fsync(") && l.contains(&fsync_da_pasta) && ok(l))
            {
                erros.push(format!(
                    "passo {n}: {arquivo} mudou e a pasta ficou sem fsync"
                ));
            }
            if n == 4 {
                // A ordem: nenhum `fsync` da pasta entre o ultimo nome da
                // tabela que saiu e o `gatilhos.json` saindo.
                let ultimo_da_tabela = janela[..evento].iter().rposition(|l| {
                    l.contains("unlink")
                        && ok(l)
                        && l.contains(&format!("\"{}", pasta.join("sai.").display()))
                });
                match ultimo_da_tabela {
                    None => erros.push("passo 4: a premissa: a tabela sai nao saiu".into()),
                    Some(u) => {
                        if janela[u + 1..evento]
                            .iter()
                            .any(|l| l.contains("fsync(") && l.contains(&fsync_da_pasta))
                        {
                            erros.push(
                                "passo 4: o sumico da tabela foi ao disco ANTES do \
                                     gatilhos.json: a queda no meio deixa o gatilho orfao"
                                    .into(),
                            );
                        }
                    }
                }
            }
        }
        assert!(erros.is_empty(), "{erros:#?}\n{texto}");
    }
}
