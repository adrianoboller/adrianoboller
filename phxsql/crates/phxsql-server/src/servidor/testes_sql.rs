//! A op `sql` -- traducao, e nao motor novo.
//!
//! O que estes testes travam:
//!
//! 1. `SELECT *` vira `varrer` e as linhas chegam;
//! 2. a projecao de colunas acontece, com o rotulo do `AS`;
//! 3. `COUNT(*)` sai do cabecalho, sem varrer;
//! 4. `WHERE col = ?` com indice vira `buscar`;
//! 5. o que NAO tem substrato recusa dizendo o que falta -- e nao devolve a
//!    tabela inteira com o filtro esquecido no caminho;
//! 6. erro de sintaxe aponta a COLUNA do texto.
use super::*;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("sql-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Uma base `b` com `clientes(id, nome, cidade)` e indice unico por id.
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
    for (id, nome, cidade) in [
        (1, "Adriano", "Blumenau"),
        (2, "Maria", "Joinville"),
        (3, "Joao", "Blumenau"),
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

fn linhas(j: &Json) -> Vec<Json> {
    j.campo("linhas").and_then(Json::lista).unwrap().to_vec()
}

#[test]
fn select_estrela_vira_varrer_e_traz_as_linhas() {
    let guarda = dir_temp("estrela");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT * FROM clientes").unwrap();
    assert_eq!(r.texto_ou("op", ""), "varrer");
    assert_eq!(linhas(&r).len(), 3);
    assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Adriano");
    // A nota nao e enfeite: sem ela, quem esperava outra ordem culpa o
    // motor em vez de escrever o ORDER BY.
    assert!(r
        .campo("notas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .any(|n| n.texto().unwrap_or("").contains("DIGITACAO")));
}

/// A projecao e do servidor porque o protocolo sempre devolve a linha
/// inteira -- o `.reg` e de slot fixo, e ler meia linha custa a mesma
/// leitura. Quem escreveu SQL espera as colunas que pediu.
#[test]
fn a_projecao_fica_so_com_as_colunas_pedidas_e_usa_o_apelido() {
    let guarda = dir_temp("projecao");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT nome AS quem, cidade FROM clientes").unwrap();
    assert_eq!(
        r.campo("colunas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|c| c.texto().unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        vec!["quem", "cidade"]
    );
    let primeira = &linhas(&r)[0];
    assert_eq!(primeira.chaves(), vec!["quem", "cidade"]);
    assert_eq!(primeira.texto_ou("quem", ""), "Adriano");
    // E o que NAO foi pedido nao vem junto -- nem a coluna de sistema.
    assert!(primeira.campo("softdeleted").is_none());
}

/// A contagem sai do cabecalho, em O(1). Varrer a tabela para contar e o
/// erro que a bancada ja cometeu uma vez.
#[test]
fn count_estrela_sai_do_cabecalho_sem_varrer() {
    let guarda = dir_temp("count");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT COUNT(*) FROM clientes").unwrap();
    assert_eq!(r.inteiro_ou("contagem", -1), 3);
    assert_eq!(r.inteiro_ou("registros", -1), 3);
}

/// **`SELECT COUNT(*)` nao pode devolver uma LINHA de dado.**
///
/// A traducao pede `max: 1` para ler o cabecalho, e a linha que vem junto e
/// efeito colateral do caminho -- nao a resposta. Devolve-la fazia o
/// console desenhar uma tabela de uma linha embaixo da contagem, e quem
/// olha nao tem como saber se aquela linha quer dizer alguma coisa.
///
/// Achado exercitando o console, e nao lendo o codigo: no JSON o campo
/// extra passa despercebido; na tela ele vira uma tabela inteira.
#[test]
fn a_contagem_nao_arrasta_a_linha_que_a_traducao_leu() {
    let guarda = dir_temp("count-limpo");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT COUNT(*) FROM clientes").unwrap();
    assert!(
        r.campo("linhas").is_none(),
        "a contagem veio com linha de dado: {}",
        r.escrever()
    );
    // E nem os campos que descrevem uma pagina que ninguem pediu.
    for campo in ["devolvidas", "cursor_inicio", "ha_mais", "ordem"] {
        assert!(
            r.campo(campo).is_none(),
            "{campo} nao descreve nada numa contagem: {}",
            r.escrever()
        );
    }
    // Ja o COUNT(*) com WHERE conta o que a busca achou.
    let r = sql(&s, "SELECT COUNT(*) FROM clientes WHERE id = 2").unwrap();
    assert_eq!(r.inteiro_ou("contagem", -1), 1);
    assert!(r.campo("linhas").is_none());
}

#[test]
fn where_com_indice_vira_buscar() {
    let guarda = dir_temp("where");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT nome FROM clientes WHERE id = 2").unwrap();
    assert_eq!(r.texto_ou("op", ""), "buscar");
    assert_eq!(linhas(&r).len(), 1);
    assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Maria");
}

/// Pedido 223, pelo caminho INTEIRO: SQL em texto, traduzido para
/// `buscar`, contra uma chave `Sequence` -- e nao a `Int4` do resto do
/// modulo. `Sequence` e o tipo da chave primaria de quase toda tabela
/// nascida pela tela, e o alargamento de tipo do tradutor (todo literal
/// numerico vira TEXTO) tinha alcancado `Int`/`UInt` e nao o irmao: um
/// `WHERE id = 2` contra `Sequence` recusava com "esperado numero da
/// sequencia, recebido Texto(\"2\")" enquanto a MESMA consulta contra
/// `Int8` passava ao lado. As duas tabelas nascem lado a lado aqui, e a
/// mesma consulta tem de trazer a mesma linha nas duas.
#[test]
fn where_sobre_chave_sequence_vira_buscar_como_o_irmao_int8() {
    let guarda = dir_temp("where-sequence");
    let c = Config {
        base: guarda.to_path_buf(),
        log_acessos: guarda.join("acessos.log"),
        blacklist: guarda.join("blacklist.json"),
        dblink: guarda.join("dblink.json"),
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
            r#"{"database":"b","tabela":"sequenciais",
                    "colunas":[{"nome":"id","tipo":"Sequence","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    // O irmao de chave Int8, para a mesma consulta ter os dois lados no
    // mesmo teste -- e nao so a memoria de que ele ja passava.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"inteiros",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for tabela in ["sequenciais", "inteiros"] {
        for (id, nome) in [(1, "Adriano"), (2, "Maria")] {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{tabela}",
                             "linha":{{"id":{id},"nome":"{nome}"}}}}"#
                )),
                &dono,
            )
            .unwrap();
        }
    }

    let r = sql(&s, "SELECT nome FROM sequenciais WHERE id = 2").unwrap();
    assert_eq!(r.texto_ou("op", ""), "buscar", "{}", r.escrever());
    assert_eq!(linhas(&r).len(), 1);
    assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Maria");

    // O irmao continua passando -- o conserto nao pode ter mudado nada
    // de quem ja funcionava.
    let r = sql(&s, "SELECT nome FROM inteiros WHERE id = 2").unwrap();
    assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Maria");
}

/// **O que nao tem substrato recusa dizendo o que falta.** `cidade` nao
/// tem indice. O `varrer` PASSOU a filtrar (`"onde"`), e mesmo assim a
/// recusa fica: ele filtra dentro da pagina que EXAMINA, e um SELECT nao
/// tem onde dizer «respondi sobre as primeiras mil». Responder sobre a
/// primeira pagina com cara de ter respondido sobre a tabela e pior que
/// recusar.
#[test]
fn where_sem_indice_recusa_em_vez_de_trazer_tudo() {
    let guarda = dir_temp("sem-indice");
    let s = servidor(&guarda);
    let e = sql(&s, "SELECT * FROM clientes WHERE cidade = 'Blumenau'").unwrap_err();
    let msg = e.to_string();
    assert!(msg.contains("cidade"), "{msg}");
    assert!(msg.contains("indice"), "{msg}");
    // E diz qual coluna TEM indice, que e o que permite consertar.
    assert!(msg.contains("id"), "{msg}");
}

/// Erro de sintaxe aponta a coluna do texto. Sem a posicao, quem escreveu
/// um comando de duzentos caracteres procura o erro no lugar errado.
#[test]
fn erro_de_sintaxe_diz_a_coluna() {
    let guarda = dir_temp("sintaxe");
    let s = servidor(&guarda);
    let msg = sql(&s, "SELECT * FRON clientes").unwrap_err().to_string();
    assert!(msg.contains("coluna"), "{msg}");
    assert!(msg.contains("FROM"), "{msg}");

    // DELETE existe desde o passo 2 do roteiro; sem WHERE ele recusa pelo
    // nome do que faltou -- e continua dizendo a coluna do texto.
    let msg = sql(&s, "DELETE FROM clientes").unwrap_err().to_string();
    assert!(msg.contains("DELETE sem WHERE"), "{msg}");
    assert!(msg.contains("coluna"), "{msg}");
}

/// O `LIMIT`/`OFFSET` chega ao `varrer` como `max` e `pular` -- e nao e
/// aplicado no cliente depois de trazer tudo.
#[test]
fn limit_e_offset_viram_max_e_pular() {
    let guarda = dir_temp("limite");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT * FROM clientes LIMIT 1 OFFSET 1").unwrap();
    assert_eq!(linhas(&r).len(), 1);
    assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Maria");
}

/// Pedido sem texto nenhum recusa dizendo o nome do campo. E `sql` e aceito
/// como sinonimo de `texto`, porque e o nome que um driver escreveria.
#[test]
fn sem_texto_recusa_com_o_nome_do_campo() {
    let guarda = dir_temp("vazio");
    let s = servidor(&guarda);
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"sql","database":"b"}"#,
        &mut ses,
        "127.0.0.1",
    );
    assert!(r.unwrap_err().to_string().contains("texto"));

    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"sql","database":"b","sql":"SELECT COUNT(*) FROM clientes"}"#,
        &mut ses,
        "127.0.0.1",
    );
    assert_eq!(r.unwrap().inteiro_ou("contagem", -1), 3);
}

/// A politica vale para a operacao TRADUZIDA, e nao so para a `sql`. Um
/// servidor que proibe `varrer` nao pode ser varrido escrevendo SELECT.
#[test]
fn a_politica_vale_para_a_operacao_traduzida() {
    let dir = dir_temp("politica");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.politica.comandos_proibidos = vec!["varrer".into()];
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
        ),
        &dono,
    )
    .unwrap();

    let e = sql(&s, "SELECT * FROM clientes").unwrap_err();
    assert!(
        e.to_string().contains("varrer") && e.to_string().contains("proibida"),
        "{e}"
    );
}

// ------------------------------------------------------- SELECT DISTINCT
//
// A traducao e provada em `phxsql-sql`; o que se prova AQUI e a costura
// -- e ela tem um defeito proprio possivel, que nenhum teste de traducao
// acusaria: o `agrupar` EXIGE um agregado, entao a resposta crua sempre
// traz uma coluna de contagem. Se a projecao do `resposta_do_sql` nao a
// jogar fora, `SELECT DISTINCT cidade` devolve `{cidade, contagem}` --
// uma coluna que ninguem pediu, com cara de dado.

#[test]
fn distinct_devolve_cada_valor_uma_vez_e_sem_a_contagem() {
    let guarda = dir_temp("distinct");
    let s = servidor(&guarda);
    // Blumenau aparece em DUAS linhas (Adriano e Joao).
    let r = sql(&s, "SELECT DISTINCT cidade FROM clientes").unwrap();
    assert_eq!(r.texto_ou("op", ""), "agrupar");
    let ls = linhas(&r);
    assert_eq!(ls.len(), 2, "{}", r.escrever());
    let mut cidades: Vec<String> = ls
        .iter()
        .map(|l| l.texto_ou("cidade", "").to_string())
        .collect();
    cidades.sort();
    assert_eq!(cidades, vec!["Blumenau", "Joinville"]);
    // A contagem que o `agrupar` carrega NAO chega a quem perguntou.
    for l in &ls {
        assert_eq!(l.chaves(), vec!["cidade"], "{}", l.escrever());
    }
    // E o cabecalho diz a mesma coisa que as linhas.
    assert_eq!(
        r.campo("colunas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|c| c.texto().unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        vec!["cidade"]
    );
}

/// Com DUAS colunas a chave e a TUPLA -- `(Blumenau, Adriano)` e
/// `(Blumenau, Joao)` sao linhas distintas, e as tres do cadastro saem
/// inteiras. Sem o `por` plural, sairiam duas.
#[test]
fn distinct_de_duas_colunas_usa_a_tupla_como_chave() {
    let guarda = dir_temp("distinct-tupla");
    let s = servidor(&guarda);
    let r = sql(&s, "SELECT DISTINCT cidade, nome FROM clientes").unwrap();
    assert_eq!(linhas(&r).len(), 3, "{}", r.escrever());
    assert_eq!(linhas(&r)[0].chaves(), vec!["cidade", "nome"]);
}

/// NULO e um valor, e dois NULOS sao a MESMA linha -- o que os quatro
/// motores fazem. Aqui isso nao e escolha desta camada: sai do
/// `phxsql_store::memoria::chave`, que da a `Value::Null` uma chave
/// propria e igual a si mesma.
#[test]
fn no_distinct_dois_nulos_sao_um_valor_so() {
    let guarda = dir_temp("distinct-nulo");
    let s = servidor(&guarda);
    let dono = Sessao::default();
    for id in [10, 11] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"clientes","linha":{{"id":{id},"nome":"X"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let r = sql(&s, "SELECT DISTINCT cidade FROM clientes").unwrap();
    // Blumenau, Joinville e UM nulo -- nao dois.
    assert_eq!(linhas(&r).len(), 3, "{}", r.escrever());
    let nulas = linhas(&r)
        .iter()
        .filter(|l| matches!(l.campo("cidade"), Some(Json::Nulo)))
        .count();
    assert_eq!(nulas, 1, "{}", r.escrever());
}

/// `ORDER BY` e `LIMIT` chegam ao `agrupar` e valem sobre o resultado JA
/// sem repetidas -- que e a ordem que o SQL manda.
#[test]
fn distinct_respeita_a_ordem_e_o_limite() {
    let guarda = dir_temp("distinct-ordem");
    let s = servidor(&guarda);
    let r = sql(
        &s,
        "SELECT DISTINCT cidade FROM clientes ORDER BY cidade DESC",
    )
    .unwrap();
    assert_eq!(linhas(&r)[0].texto_ou("cidade", ""), "Joinville");
    let r = sql(
        &s,
        "SELECT DISTINCT cidade FROM clientes ORDER BY cidade LIMIT 1",
    )
    .unwrap();
    assert_eq!(linhas(&r).len(), 1);
    assert_eq!(linhas(&r)[0].texto_ou("cidade", ""), "Blumenau");
}

/// **O DISTINCT herda o TETO do `agrupar`, e a recusa DIZ isso.**
///
/// Os grupos ficam TODOS em memoria ao mesmo tempo -- e o grupo de um
/// DISTINCT e uma linha do resultado. Uma coluna de alta cardinalidade
/// para no teto, e a recusa nomeia `recursos.max_linhas` e o que fazer,
/// em vez de o servidor encher a memoria calado.
#[test]
fn o_distinct_herda_o_teto_do_agrupar_e_a_recusa_o_nomeia() {
    let guarda = dir_temp("distinct-teto");
    let c = Config {
        base: guarda.to_path_buf(),
        log_acessos: guarda.join("acessos.log"),
        blacklist: guarda.join("blacklist.json"),
        dblink: guarda.join("dblink.json"),
        token: "t".into(),
        // Dois grupos cabem; o terceiro nao.
        max_linhas: 2,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"cidade","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for (id, cidade) in [(1, "A"), (2, "B"), (3, "C")] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"clientes",
                         "linha":{{"id":{id},"cidade":"{cidade}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let e = sql(&s, "SELECT DISTINCT cidade FROM clientes")
        .unwrap_err()
        .to_string();
    assert!(e.contains("recursos.max_linhas"), "{e}");
    assert!(e.contains("memoria"), "{e}");
}

/// **O `ORDER BY` de um DISTINCT aceita a COLUNA e o APELIDO**, porque os
/// dois querem dizer a mesma coluna -- e os dois chegam ao `agrupar` como
/// o nome que ele conhece, que e o da coluna. O apelido so existe na
/// projecao de SAIDA, que acontece depois: mandar `praca` no `"ordem"`
/// faria o motor recusar falando de `"por"` e de apelido de agregado,
/// vocabulario que quem escreveu SQL nao tem como reconhecer.
#[test]
fn a_ordem_do_distinct_aceita_a_coluna_e_o_apelido() {
    let guarda = dir_temp("distinct-apelido");
    let s = servidor(&guarda);
    for texto in [
        "SELECT DISTINCT cidade AS praca FROM clientes ORDER BY praca DESC",
        "SELECT DISTINCT cidade AS praca FROM clientes ORDER BY cidade DESC",
    ] {
        let r = sql(&s, texto).unwrap_or_else(|e| panic!("{texto}: {e}"));
        assert_eq!(linhas(&r).len(), 2, "{texto}: {}", r.escrever());
        // O rotulo de SAIDA e o apelido...
        assert_eq!(linhas(&r)[0].chaves(), vec!["praca"], "{texto}");
        // ...e a ordem foi mesmo aplicada.
        assert_eq!(linhas(&r)[0].texto_ou("praca", ""), "Joinville", "{texto}");
    }
}

/// O `WHERE` peneira ANTES de eliminar repetido, como em todo SQL.
#[test]
fn o_where_do_distinct_peneira_antes_de_eliminar_repetido() {
    let guarda = dir_temp("distinct-where");
    let s = servidor(&guarda);
    let r = sql(
        &s,
        "SELECT DISTINCT cidade FROM clientes WHERE nome = 'Joao'",
    )
    .unwrap();
    assert_eq!(linhas(&r).len(), 1);
    assert_eq!(linhas(&r)[0].texto_ou("cidade", ""), "Blumenau");
}

// --------------------------------------------------- UNION e UNION ALL
//
// A op `unir` ja existia. O que se prova AQUI e a costura -- e ela tem um
// defeito proprio que nenhum teste de traducao acusaria: o `unir`
// responde `linhas` em LISTA posicional e `colunas` em `[{nome,tipo,…}]`,
// e nenhum outro SELECT responde assim. Quem pergunta em SQL nao pode
// receber um envelope diferente por causa da operacao que atendeu.

/// Uma segunda tabela de mesmo esquema, com uma linha repetida da
/// primeira -- e na SEGUNDA posicao, para que os `rownum` NAO casem. Foi
/// exatamente o `rownum` casando por acaso que escondeu o defeito do
/// `UNION` por tanto tempo: unir duas tabelas identicas desduplica
/// perfeitamente, e o caso obvio passa por coincidencia.
fn segunda_tabela(s: &Arc<Servidor>) {
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes2",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"cidade","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId2","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for (id, nome, cidade) in [(9, "Nova", "Itajai"), (1, "Adriano", "Blumenau")] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"clientes2",
                         "linha":{{"id":{id},"nome":"{nome}","cidade":"{cidade}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
}

/// **`UNION` tira a repetida e `UNION ALL` nao** -- e a linha repetida
/// esta em posicoes diferentes das duas tabelas, entao o `rownum` dela
/// nao casa. Com a chave lendo as colunas de sistema, os dois comandos
/// devolviam a MESMA coisa.
#[test]
fn union_tira_a_repetida_e_union_all_nao_pela_op_sql() {
    let guarda = dir_temp("union");
    let s = servidor(&guarda);
    segunda_tabela(&s);

    let r = sql(&s, "SELECT * FROM clientes UNION SELECT * FROM clientes2").unwrap();
    assert_eq!(r.texto_ou("op", ""), "unir");
    assert_eq!(linhas(&r).len(), 4, "{}", r.escrever());
    assert_eq!(r.inteiro_ou("repetidas", -1), 1, "{}", r.escrever());

    let r = sql(
        &s,
        "SELECT * FROM clientes UNION ALL SELECT * FROM clientes2",
    )
    .unwrap();
    assert_eq!(linhas(&r).len(), 5, "{}", r.escrever());
    assert_eq!(r.inteiro_ou("repetidas", -1), 0);
}

/// **A resposta sai no envelope de todo SELECT.** O `unir` devolve lista
/// de listas e um cabecalho `[{nome,tipo,lado,chave}]`; quem escreveu SQL
/// recebe objetos nomeados e `colunas` como lista de rotulos -- o mesmo
/// contrato de `SELECT * FROM t`. E as colunas de sistema nao aparecem,
/// porque o proprio `unir` as tira.
#[test]
fn a_uniao_responde_no_envelope_de_todo_select() {
    let guarda = dir_temp("union-envelope");
    let s = servidor(&guarda);
    segunda_tabela(&s);
    let r = sql(
        &s,
        "SELECT * FROM clientes UNION ALL SELECT * FROM clientes2",
    )
    .unwrap();
    assert_eq!(
        r.campo("colunas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|c| c.texto().unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        vec!["id", "nome", "cidade"],
        "{}",
        r.escrever()
    );
    let primeira = &linhas(&r)[0];
    assert_eq!(primeira.chaves(), vec!["id", "nome", "cidade"]);
    assert_eq!(primeira.texto_ou("nome", ""), "Adriano");
    assert_eq!(primeira.inteiro_ou("id", -1), 1);
    // E o campo `sql` aparece UMA vez, com o TEXTO do comando -- o
    // rotulo "UNION ALL" que o `unir` responde no campo de mesmo nome
    // nao vem junto. Duas chaves iguais num objeto JSON fazem quem le
    // ver uma sem saber qual.
    assert_eq!(
        r.chaves().iter().filter(|k| **k == "sql").count(),
        1,
        "{}",
        r.escrever()
    );
    assert_eq!(
        r.texto_ou("sql", ""),
        "SELECT * FROM clientes UNION ALL SELECT * FROM clientes2"
    );
    assert_eq!(r.texto_ou("modo", ""), "tudo");
}

/// O que NAO tem substrato recusa NOMEANDO o que existe -- e a frase diz
/// a forma que passa, em vez de um "sintaxe invalida".
#[test]
fn a_uniao_sem_substrato_recusa_nomeando_pela_op_sql() {
    let guarda = dir_temp("union-recusa");
    let s = servidor(&guarda);
    segunda_tabela(&s);
    let e = sql(
        &s,
        "SELECT nome FROM clientes UNION SELECT nome FROM clientes2",
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("TABELAS inteiras"), "{e}");
    assert!(e.contains("SELECT * FROM a UNION"), "{e}");
}

/// Esquemas que nao empilham recusam com a CONTA das colunas dos dois
/// lados -- quem le sabe qual e a diferenca sem abrir o esquema.
#[test]
fn a_uniao_de_esquemas_que_nao_empilham_recusa_com_a_conta() {
    let guarda = dir_temp("union-esquema");
    let s = servidor(&guarda);
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"curta",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porIdC","colunas":["id"],"unico":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    let e = sql(&s, "SELECT * FROM clientes UNION SELECT * FROM curta")
        .unwrap_err()
        .to_string();
    assert!(e.contains("coluna(s)"), "{e}");
}
