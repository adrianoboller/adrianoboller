//! 4. a arvore e o catalogo **escondem** o que nao da para abrir;
//! 5. um `config.json` sem regra de tabela continua se comportando igual.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("dt-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// O cadastro sai do JSON, e nao de uma struct montada a mao: assim o
/// teste tambem exercita a LEITURA do `config.json`, que e onde o direito
/// por tabela e escrito de verdade.
fn cadastro(bases: &str) -> Cadastro {
    Cadastro::de_json(&pedido(&format!(
        r#"{{"usuarios":[{{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
    )))
    .unwrap()
}

/// Uma base `b` com duas tabelas: `clientes` e `folha`.
fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> (Arc<Servidor>, Sessao) {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: cadastro.clone(),
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
                r#"{{"database":"b","tabela":"{tab}",
                        "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                                   {{"nome":"nome","tipo":"Str(20)"}}],
                        "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                     "primario":true}}]}}"#
            )),
            &dono,
        )
        .unwrap();
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{tab}","linha":{{"id":1,"nome":"x"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade: e ali que
/// mora o portao, e nao no `executar`.
fn pede(s: &Arc<Servidor>, sessao: &Sessao, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        usuario: sessao.usuario.clone(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// O caso do enunciado: le a base inteira, menos a folha.
#[test]
fn a_regra_da_tabela_tira_de_quem_le_a_base() {
    let dir = dir_temp("tira");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );

    assert!(pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#
    )
    .is_ok());
    let e = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO");
    assert!(
        e.to_string().contains("b.folha"),
        "a recusa tem de dizer QUAL tabela: {e}"
    );
}

/// E o contrario, que e o caso que a intersecao nao resolveria: nao le a
/// base nenhuma, le uma tabela.
#[test]
fn a_regra_da_tabela_da_a_quem_nao_le_a_base() {
    let dir = dir_temp("da");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"b":{"tabelas":{"clientes":{"ler":true}}}}"#),
    );

    assert!(pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#
    )
    .is_ok());
    assert!(pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#
    )
    .is_err());
    // E continua sem poder GRAVAR na que le.
    assert!(pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":2}"#
    )
    .is_err());
}

/// `"*"` de tabela vale para as nao listadas, como o `"*"` de base.
#[test]
fn a_estrela_de_tabela_vale_para_as_nao_listadas() {
    let dir = dir_temp("estrela");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"*":{},"clientes":{"ler":true}}}}"#),
    );
    assert!(pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#
    )
    .is_ok());
    assert!(pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#
    )
    .is_err());
}

/// **O `procurar_texto` passa pelo portao UNICO**, e passa porque nasceu
/// com o campo `"tabela"` no primeiro nivel -- que e o campo que o portao
/// le.
///
/// A petrea manda procurar, a cada operacao nova, quem NAO tem esse campo:
/// as tres que escondem a tabela dele (`juntar`, `unir`, `pivotar`) pagam
/// conferencia propria. Esta nao esconde, entao nao paga -- e este teste e
/// o que prova que ela nao esconde.
#[test]
fn procurar_texto_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
    let dir = dir_temp("fts-portao");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"clientes":{"ler":true}}}}"#),
    );
    let erro = pede(
        &s,
        &ses,
        r#""op":"procurar_texto","database":"b","tabela":"folha","indice":"porNome","palavra":"ana""#,
    )
    .expect_err("o procurar_texto leu a tabela negada");
    assert!(
        erro.to_string().to_lowercase().contains("folha")
            || erro.to_string().to_lowercase().contains("permiss")
            || erro.to_string().to_lowercase().contains("direito"),
        "a recusa tem de dizer por que: {erro}"
    );
    // E na tabela PERMITIDA a recusa nao pode ser a do portao: ela tem de
    // chegar ao motor, que responde que nao ha indice de texto declarado.
    let r = pede(
        &s,
        &ses,
        r#""op":"procurar_texto","database":"b","tabela":"clientes","indice":"porNome","palavra":"ana""#,
    );
    let recado = match &r {
        Ok(_) => String::new(),
        Err(e) => e.to_string().to_lowercase(),
    };
    assert!(
        !recado.contains("permiss") && !recado.contains("direito"),
        "a tabela permitida foi barrada pelo portao: {recado}"
    );
}

/// **O indice de TEXTO era o mesmo oraculo, por uma chave que a
/// conferencia nao sabia resolver.** `procurar_texto` nomeia o indice no
/// MESMO campo `"indice"` que o `buscar`, e
/// `recusar_pergunta_sobre_coluna_negada` le esse mesmo campo -- mas
/// resolvia o nome por `colunas_do_indice`, que varria so `indices`, as
/// arvores. O indice de texto sai do esquema em `indices_texto`, e com a
/// coluna por NOME: para um nome de la a conferencia devolvia duas listas
/// vazias, nenhum dos dois lacos rodava, e ela retornava `Ok(())` --
/// **passava vazia**.
///
/// O irmao estava fechado, e e isso que prova esquecimento e nao decisao:
/// `buscar` por um indice de arvore sobre a coluna negada recusa desde
/// sempre, porque aquele nome mora em `indices`. O mesmo pedido pela
/// porta do `.fts` passava.
///
/// O que vazava: a peneira tira `nome` de dentro de `linhas`, mas
/// `encontrados` e o `rowid` sobrevivem ao lado dela, e o casamento ja
/// aconteceu. Com o rowid na mao nao sao vinte perguntas sobre a base --
/// sao vinte sobre AQUELA pessoa.
///
/// LASTRO, e ele e o motivo de este teste criar tabela propria: o
/// `servidor()` desta bateria nao declara indice de texto em lugar
/// nenhum, e o irmao logo acima prova o portao de TABELA por uma tabela
/// que nem tem `.fts` -- ela responde «nao ha indice de texto
/// declarado». Um teste de coluna escrito sobre aquela fixture passaria
/// pelo motivo ERRADO, pela recusa do MOTOR, num pedido cujo assunto e
/// justamente uma conferencia que passa vazia. Por isso a terceira
/// assercao nega a frase do motor: ela diz de ONDE veio a recusa.
///
/// PROVA REAL: sem o laco de `indices_texto` em `colunas_do_indice`, o
/// primeiro pedido volta `Ok` com `encontrados: 1` -- que E a resposta
/// que se queria esconder: «sim, ha uma ficha com a palavra "ana" na
/// coluna que este usuario nao le», com o rowid ao lado.
#[test]
fn o_indice_de_texto_nao_pergunta_pela_coluna_negada() {
    let dir = dir_temp("fts-coluna");
    let (s, ses) = servidor(
        &dir,
        cadastro(
            r#"{"*":{"ler":true,"tabelas":{"fichas":{"ler":true,
                     "colunas":{"nome":{"ler":false}}}}}}"#,
        ),
    );
    // A tabela com `.fts` nasce AQUI, e nao na fixture: mexer nela
    // mudaria o que o irmao logo acima prova.
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"fichas",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"obs","tipo":"Str(40)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}],
                    "indices_texto":[{"nome":"porNome","coluna":"nome"},
                                     {"nome":"porObs","coluna":"obs"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"b","tabela":"fichas",
                    "linha":{"id":1,"nome":"ana","obs":"cliente antigo"}}"#,
        ),
        &dono,
    )
    .unwrap();

    let e = pede(
        &s,
        &ses,
        r#""op":"procurar_texto","database":"b","tabela":"fichas","indice":"porNome","palavra":"ana""#,
    )
    .expect_err("o indice de texto respondeu sobre a coluna negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    let t = e.to_string();
    assert!(
        t.contains("nome") && t.contains("o indice"),
        "a recusa tem de dizer QUAL coluna e por ONDE: {t}"
    );
    // De onde veio a recusa: do PORTAO, e nao do motor dizendo que o
    // indice de texto nao existe. Sem esta linha o teste passaria pelo
    // lastro, e nao pela guarda.
    assert!(
        !t.contains("nao tem indice de texto"),
        "a recusa e do motor, e nao do portao: {t}"
    );

    // O indice de texto sobre coluna que se PODE ler continua servindo --
    // guarda que recusasse tudo seria pior que a guarda que faltava --, e
    // a peneira continua tirando a coluna negada da resposta.
    let r = pede(
        &s,
        &ses,
        r#""op":"procurar_texto","database":"b","tabela":"fichas","indice":"porObs","palavra":"cliente""#,
    )
    .expect("o indice de texto sobre coluna permitida foi barrado");
    assert_eq!(r.inteiro_ou("encontrados", -1), 1, "{r:?}");
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    assert!(linhas[0].campo("nome").is_none(), "a coluna vazou: {r:?}");
}

/// A arvore mostra o que da para abrir, e nao o que existe.
#[test]
fn a_arvore_esconde_a_tabela_negada() {
    let dir = dir_temp("arvore");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    let r = pede(&s, &ses, r#""op":"tabelas","database":"b""#).unwrap();
    let nomes: Vec<String> = r
        .campo("tabelas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(|x| x.texto().map(str::to_string))
        .collect();
    assert_eq!(nomes, vec!["clientes".to_string()], "veio {nomes:?}");
}

/// **A op `sql` NAO e a porta dos fundos.** Ana le `clientes` e nao le
/// `folha`; escrever o nome da folha dentro de um SELECT nao muda isso.
///
/// Este e o teste que importa do item inteiro: se ele passar a falhar,
/// alguem trocou o `executar_derivado` por uma leitura direta da tabela.
#[test]
fn o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
    let dir = dir_temp("sql-porta");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );

    let ok = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"SELECT * FROM clientes""#,
    )
    .expect("a tabela permitida tinha de passar");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 1);

    let e = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"SELECT * FROM folha""#,
    )
    .expect_err("o SELECT leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");
}

/// A escrita pelo SQL passa pelo MESMO portao da leitura: quem nao pode
/// gravar na folha tambem nao grava nela escrevendo INSERT, UPDATE ou
/// DELETE -- e a recusa vem antes de qualquer passo tocar o disco.
#[test]
fn o_dml_pelo_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
    let dir = dir_temp("sql-dml-porta");
    let (s, ses) = servidor(
        &dir,
        cadastro(
            r#"{"*":{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                     "tabelas":{"folha":{}}}}"#,
        ),
    );
    let corpo = |sql: &str| {
        format!(
            r#""op":"sql","database":"b","texto":{}"#,
            Json::texto_de(sql).escrever()
        )
    };
    for (sql, op) in [
        ("INSERT INTO clientes (id, nome) VALUES (2, 'y')", "inserir"),
        ("UPDATE clientes SET nome = 'z' WHERE id = 1", "atualizar"),
        ("DELETE FROM clientes WHERE id = 2", "excluir"),
    ] {
        let r = pede(&s, &ses, &corpo(sql))
            .unwrap_or_else(|e| panic!("{sql}: a tabela permitida tinha de passar: {e}"));
        assert_eq!(r.texto_ou("op", ""), op, "{sql}");
        assert_eq!(r.inteiro_ou("afetadas", -1), 1, "{sql}");
    }
    for sql in [
        "INSERT INTO folha (id, nome) VALUES (2, 'y')",
        "UPDATE folha SET nome = 'z' WHERE id = 1",
        "DELETE FROM folha WHERE id = 1",
    ] {
        let e = pede(&s, &ses, &corpo(sql)).expect_err("gravou na tabela negada");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{sql}: {e}");
        assert!(format!("{e}").contains("folha"), "{sql}: {e}");
    }
    // E a folha continua como estava.
    let l = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"folha","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(l.texto_ou("nome", ""), "x");
}

/// O endereco de tres partes -- `banco.schema.tabela` -- tambem nao
/// contorna nada: a permissao e conferida contra o banco que o SELECT
/// escolheu, e nao contra o do envelope. Sem isto, o campo `database` do
/// pedido seria enfeite e o SQL escolheria sozinho onde ler.
#[test]
fn o_banco_do_from_e_o_banco_da_permissao() {
    let dir = dir_temp("sql-from-db");
    let (s, ses) = servidor(&dir, cadastro(r#"{"b":{"ler":true}}"#));
    let e = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"SELECT * FROM outra.filial.clientes""#,
    )
    .expect_err("leu de um banco que nao esta na regra");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("outra"), "{e}");
}

/// A op `catalogo` mostra so o que a sessao consegue chamar.
///
/// Um leitor nao pode ver `excluir_tabela` na lista: oferecer a operacao
/// que o portao vai negar e mandar o cliente montar um pedido para ouvir
/// nao. E `ler`, que ele pode, tem de estar la -- esconder demais seria o
/// mesmo estrago do outro lado.
#[test]
fn o_catalogo_lista_so_o_que_a_sessao_pode_chamar() {
    let dir = dir_temp("cat-op");
    let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true}}"#));
    let r = pede(&s, &ses, r#""op":"catalogo","database":"b""#).unwrap();
    let nomes: Vec<String> = r
        .campo("operacoes")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|o| o.texto_ou("nome", "").to_string())
        .collect();
    assert!(nomes.contains(&"ler".to_string()), "{nomes:?}");
    assert!(nomes.contains(&"varrer".to_string()), "{nomes:?}");
    assert!(
        !nomes.contains(&"excluir_tabela".to_string()),
        "o leitor viu uma operacao de administrador: {nomes:?}"
    );
    assert!(
        !nomes.contains(&"inserir".to_string()),
        "o leitor viu uma operacao de escrita: {nomes:?}"
    );
    // E o numero do que ficou de fora, para quem ve a lista curta saber
    // que ela e curta por permissao, e nao por o servidor ser pequeno.
    assert!(r.inteiro_ou("ocultas", 0) > 0);
    assert_eq!(r.inteiro_ou("total", -1), nomes.len() as i64);
}

/// `catalogo` com `"op"` detalha uma so -- e e o que o `/help <comando>`
/// do console usa. Operacao que o usuario nao pode chamar responde POR QUE,
/// em vez de fingir que nao existe: fingir manda procurar erro de
/// digitacao onde nao ha.
#[test]
fn o_catalogo_detalha_uma_operacao_e_diz_por_que_negou() {
    let dir = dir_temp("cat-uma");
    let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true}}"#));

    let uma = pede(
        &s,
        &ses,
        r#""op":"catalogo","database":"b","operacao":"buscar""#,
    )
    .unwrap()
    .campo("operacao")
    .cloned()
    .unwrap();
    assert_eq!(uma.texto_ou("nome", ""), "buscar");
    assert!(!uma.texto_ou("exemplo", "").is_empty());
    assert!(uma
        .campo("parametros")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .any(|p| p.texto_ou("nome", "") == "indice"));

    let negada = pede(
        &s,
        &ses,
        r#""op":"catalogo","database":"b","operacao":"excluir_tabela""#,
    )
    .unwrap();
    assert!(negada.campo("operacao").unwrap().e_nulo());
    assert!(
        negada.texto_ou("motivo", "").contains("administrar"),
        "{}",
        negada.escrever()
    );
}

/// O catalogo e a mesma lista por outra porta.
#[test]
fn o_catalogo_esconde_a_tabela_negada() {
    let dir = dir_temp("catalogo");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    for op in ["sistabelas", "siscolunas"] {
        let r = pede(&s, &ses, &format!(r#""op":"{op}","database":"b""#)).unwrap();
        let texto = r.escrever();
        assert!(
            !texto.contains("folha"),
            "{op} vazou a tabela negada: {texto}"
        );
        assert!(texto.contains("clientes"), "{op} escondeu demais");
    }
}

/// Junção nao tem campo `tabela`: as duas moram em `a.tabela` e `b.tabela`.
/// Sem a conferencia propria, bastaria pedir a folha como lado B.
#[test]
fn juntar_nao_e_a_porta_dos_fundos() {
    let dir = dir_temp("juntar");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    let e = pede(
        &s,
        &ses,
        r#""op":"juntar","database":"b",
               "a":{"tabela":"clientes","chave":"id"},
               "b":{"tabela":"folha","chave":"id"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO");
    assert!(e.to_string().contains("b.folha"), "veio {e}");
}

/// União tambem nao tem campo `tabela`: tem uma LISTA.
#[test]
fn unir_nao_e_a_porta_dos_fundos() {
    let dir = dir_temp("unir");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    let e = pede(
        &s,
        &ses,
        r#""op":"unir","database":"b","tabelas":["clientes","folha"]"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO");
    assert!(e.to_string().contains("b.folha"), "veio {e}");
}

/// **E o braco-PEDIDO da uniao tambem nao e** (pedido 393). Aqui a
/// recusa NAO vem da conferencia propria do `unir`: vem do
/// `executar_derivado`, que e o irmao do `despachar` -- cada braco paga o
/// portao da tabela DELE, como qualquer `varrer` que chegasse pela rede.
///
/// **O teste existe porque a forma do braco mudou**, e mudanca de forma e
/// exatamente quando uma porta dos fundos nasce: o campo `"tabelas"` que
/// a conferencia propria le nao existe num pedido com `"partes"`, e sem
/// o `executar_derivado` no meio a folha sairia inteira.
///
/// # Qual das tres asserções DISCRIMINA, e por que as outras duas nao
///
/// Medido em 23/09/2026 com o defeito reposto (`self.executar` no lugar do
/// `self.executar_derivado`, dentro do `linhas_do_sub_pedido`): as duas
/// primeiras **passaram mesmo assim**, e passaram DEPOIS do dano. O braco
/// `varrer` le a folha inteira sem portao nenhum, e so entao o
/// `linhas_do_sub_pedido` pede o `modelo_da_tabela`, que chama
/// `executar_derivado("esquema")` -- e e AI que a recusa aparece. A linha
/// ja foi lida; o que para e a resposta. O braco `consultar` recusa pelo
/// mesmo acidente, um nivel mais fundo.
///
/// O terceiro caso existe por isso: `agrupar` nao passa pelo
/// `modelo_da_tabela` (o modelo dele sai do proprio cabecalho `colunas`),
/// entao ele **nao tem recusa tardia nenhuma** -- com o defeito, a folha
/// resumida sai. **Ele e a unica das tres que cai quando a guarda cai**, e
/// isso fica escrito aqui em vez de escondido: teste que passa por engano
/// e pior que teste que falta, e as duas primeiras passam por engano
/// quando o assunto e ESTE defeito.
///
/// **Prova real, com o defeito reposto:** troque o
/// `self.executar_derivado(...)` do `linhas_do_sub_pedido` por
/// `self.executar(...)` e o caso do `agrupar` devolve a folha.
#[test]
fn o_braco_pedido_da_uniao_tambem_nao_e_a_porta_dos_fundos() {
    let dir = dir_temp("unir-partes");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    let e = pede(
        &s,
        &ses,
        r#""op":"unir","database":"b",
               "partes":[{"op":"varrer","tabela":"clientes"},
                         {"op":"varrer","tabela":"folha"}]"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(e.to_string().contains("b.folha"), "veio {e}");

    // E o braco ESCONDIDO mais fundo -- dentro do `de` de um `consultar`
    // -- recusa igual: a varredura desce porque o portao desce junto.
    let e = pede(
        &s,
        &ses,
        r#""op":"unir","database":"b",
               "partes":[{"op":"varrer","tabela":"clientes"},
                         {"op":"consultar","de":{"op":"varrer","tabela":"folha"}}]"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(e.to_string().contains("b.folha"), "veio {e}");

    // E o braco que NAO pede modelo de tabela -- `agrupar` monta o
    // modelo do proprio cabecalho `colunas` --, que e o unico dos tres
    // sem recusa tardia para o encobrir. Ver a nota do cabecalho.
    let e = pede(
        &s,
        &ses,
        r#""op":"unir","database":"b","modo":"tudo",
               "partes":[{"op":"agrupar","tabela":"clientes","por":["nome"]},
                         {"op":"agrupar","tabela":"folha","por":["nome"]}]"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(e.to_string().contains("b.folha"), "veio {e}");

    // E o que ele PODE ler continua passando: uniao de clientes com
    // clientes responde, para a recusa acima ser sobre a folha e nao
    // sobre a forma nova.
    pede(
        &s,
        &ses,
        r#""op":"unir","database":"b","modo":"tudo",
               "partes":[{"op":"varrer","tabela":"clientes"},
                         {"op":"varrer","tabela":"clientes"}]"#,
    )
    .unwrap();
}

/// O pivot tem DOIS lugares com tabela, e o portao geral so ve um: a
/// tabela de fatos em `tabela`, e a lista `juntar`, com um campo `tabela`
/// DENTRO de cada item. Sem a conferencia propria, bastava juntar a folha
/// e pedir `f.nome` em `linhas` -- os rotulos das linhas do cruzamento SAO
/// os valores da tabela negada.
///
/// A mesma falha do `juntar` e do `unir`, na terceira operacao que nao tem
/// o campo que o portao le.
#[test]
fn pivotar_nao_e_a_porta_dos_fundos() {
    let dir = dir_temp("pivotar");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    let e = pede(
        &s,
        &ses,
        r#""op":"pivotar","database":"b","tabela":"clientes",
               "juntar":[{"tabela":"folha","coluna":"id","prefixo":"f","chave":"id"}],
               "linhas":[{"campo":"f.nome"}],
               "colunas":[{"campo":"nome"}],
               "agregador":"contagem""#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(e.to_string().contains("b.folha"), "veio {e}");
}

/// E a tabela de FATOS do pivot continua passando pelo portao de sempre --
/// ela tem o campo `tabela`, entao a conferencia nova nao pode ser a
/// unica.
#[test]
fn pivotar_na_tabela_permitida_continua_valendo() {
    let dir = dir_temp("pivot-ok");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    assert!(
        pede(
            &s,
            &ses,
            r#""op":"pivotar","database":"b","tabela":"clientes",
                   "linhas":[{"campo":"nome"}],
                   "colunas":[{"campo":"id"}],
                   "agregador":"contagem""#,
        )
        .is_ok(),
        "a conferencia nova barrou um pivot legitimo"
    );
}

/// `sequencias` e a TERCEIRA porta para a lista que a arvore esconde --
/// depois do `sistabelas` e do `siscolunas`, que ja filtravam. Ela varre a
/// base inteira e nao tem campo `tabela`, entao o portao geral so ve a
/// base: sem filtro proprio, ela entregava o nome da tabela negada, o
/// contador dela e quantas linhas ela tem.
#[test]
fn sequencias_esconde_a_tabela_negada() {
    let dir = dir_temp("seq");
    let (s, ses) = servidor(
        &dir,
        cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
    );
    let r = pede(&s, &ses, r#""op":"sequencias","database":"b""#).unwrap();
    let nomes: Vec<String> = r
        .campo("sequencias")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|x| x.texto_ou("tabela", "").to_string())
        .collect();
    assert_eq!(nomes, vec!["clientes".to_string()], "veio {nomes:?}");
    assert_eq!(r.inteiro_ou("total", -1), 1, "o total ainda conta a negada");
}

/// `posicao` e o `SHOW MASTER STATUS` daqui, e tambem varre a base inteira
/// sem campo `tabela`. Com `com_esquema`, ela devolve o esquema CRU de
/// cada tabela -- entao o vazamento nao era so o nome.
///
/// A conferencia e de `replicar`, e nao de `ler`: e o direito que o portao
/// geral aplicou a operacao.
#[test]
fn posicao_esconde_a_tabela_negada() {
    let dir = dir_temp("posic");
    let (s, ses) = servidor(
        &dir,
        cadastro(
            r#"{"*":{"ler":true,"replicar":true,
                     "tabelas":{"folha":{"ler":true}}}}"#,
        ),
    );
    let r = pede(
        &s,
        &ses,
        r#""op":"posicao","database":"b","com_esquema":true"#,
    )
    .unwrap();
    // `tabelas` aqui e um OBJETO: a chave e o nome da tabela.
    let Some(Json::Objeto(pares)) = r.campo("tabelas") else {
        panic!("posicao sem o objeto tabelas: {}", r.escrever());
    };
    let nomes: Vec<String> = pares.iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(
        nomes,
        vec!["clientes".to_string()],
        "a folha, que ele le mas nao replica, apareceu: {nomes:?}"
    );
    let cru = r.escrever();
    assert!(
        !cru.contains("folha"),
        "o nome da tabela negada vazou na resposta: {cru}"
    );
}

/// E o teste que mais importa nas duas mudancas acima: a REPLICA de sempre
/// nao tem regra por tabela, e continua vendo tudo. Guarda nova que quebra
/// quem ja funcionava nao e guarda, e estrago.
#[test]
fn sem_regra_de_tabela_posicao_e_sequencias_veem_tudo() {
    let dir = dir_temp("replica-velha");
    let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true,"replicar":true}}"#));
    let r = pede(&s, &ses, r#""op":"sequencias","database":"b""#).unwrap();
    assert_eq!(r.inteiro_ou("total", -1), 2, "sequencias perdeu uma tabela");
    let r = pede(&s, &ses, r#""op":"posicao","database":"b""#).unwrap();
    let Some(Json::Objeto(pares)) = r.campo("tabelas") else {
        panic!("posicao sem o objeto tabelas: {}", r.escrever());
    };
    assert_eq!(pares.len(), 2, "posicao perdeu uma tabela");
}

/// `duplicar_tabela` cria uma tabela com o nome do campo `destino`, e o
/// portao geral confere o campo `tabela` -- que aqui e a ORIGEM. Quem
/// podia criar nominalmente UMA tabela criava qualquer outra duplicando a
/// permitida. O `copiar_tabela` ao lado ja conferia o destino dele.
#[test]
fn duplicar_confere_o_direito_no_destino() {
    let dir = dir_temp("dup");
    let (s, ses) = servidor(
        &dir,
        cadastro(
            r#"{"*":{"ler":true,
                     "tabelas":{"clientes":{"ler":true,"criar":true},"*":{"ler":true}}}}"#,
        ),
    );
    let e = pede(
        &s,
        &ses,
        r#""op":"duplicar_tabela","database":"b","tabela":"clientes",
               "destino":"clientes_copia""#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(
        e.to_string().contains("clientes_copia"),
        "a recusa tem de dizer o nome que ele tentou criar: {e}"
    );
}

/// E quem PODE criar no destino continua duplicando. Guarda que so nega
/// nao e guarda, e parede.
#[test]
fn duplicar_com_direito_no_destino_continua_valendo() {
    let dir = dir_temp("dup-ok");
    let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true,"criar":true}}"#));
    assert!(pede(
        &s,
        &ses,
        r#""op":"duplicar_tabela","database":"b","tabela":"clientes",
               "destino":"clientes_copia""#
    )
    .is_ok());
}

/// O `config.json` que ja existia continua se comportando igual. E o teste
/// que importa: uma regra nova que muda o significado da configuracao
/// antiga tira o direito de alguem sem ninguem ter pedido.
#[test]
fn sem_regra_de_tabela_nada_muda() {
    let dir = dir_temp("igual");
    let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true}}"#));
    for tab in ["clientes", "folha"] {
        assert!(
            pede(
                &s,
                &ses,
                &format!(r#""op":"ler","database":"b","tabela":"{tab}","rowid":1"#)
            )
            .is_ok(),
            "{tab} deixou de ser legivel"
        );
    }
    let r = pede(&s, &ses, r#""op":"tabelas","database":"b""#).unwrap();
    assert_eq!(r.campo("tabelas").and_then(Json::lista).unwrap().len(), 2);
}

/// Supervisor passa por cima de qualquer regra de tabela -- como ja passa
/// por cima da regra de base.
#[test]
fn supervisor_passa_por_cima() {
    let dir = dir_temp("super");
    let c = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,"supervisor":true,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&dir, c);
    assert!(pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#
    )
    .is_ok());
}

// ---------------------------------------------------------------------
// Pedido 775 -- o `bancos` filtra pela ficha, como o `tabelas`.
// ---------------------------------------------------------------------

/// Duas bases vazias, `loja` e `rh`, e cinco fichas: so a `loja`, uma
/// tabela so do `rh`, o curinga `"*"`, so o aquario e a supervisora.
fn servidor_de_duas_bases(dir: &std::path::Path) -> (Arc<Servidor>, Cadastro) {
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[
             {"login":"lojaadm","id":11,"senha_hash":"pbkdf2-sha256$1000$00$00",
              "bases":{"loja":{"ler":true,"inserir":true,"administrar":true}}},
             {"login":"so_tabela","id":12,"senha_hash":"pbkdf2-sha256$1000$00$00",
              "bases":{"rh":{"tabelas":{"ponto":{"ler":true}}}}},
             {"login":"curinga","id":13,"senha_hash":"pbkdf2-sha256$1000$00$00",
              "bases":{"*":{"ler":true}}},
             {"login":"so_aquario","id":14,"senha_hash":"pbkdf2-sha256$1000$00$00",
              "bases":{"*":{"monitorar":true}}},
             {"login":"chefe","id":15,"supervisor":true,
              "senha_hash":"pbkdf2-sha256$1000$00$00"}]}"#,
    ))
    .unwrap();
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: cadastro.clone(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    for b in ["loja", "rh"] {
        s.executar(
            "criar_database",
            &pedido(&format!(r#"{{"database":"{b}"}}"#)),
            &Sessao::default(),
        )
        .unwrap();
    }
    (s, cadastro)
}

fn bancos_de(s: &Arc<Servidor>, cadastro: &Cadastro, login: &str) -> Result<Vec<String>> {
    let ses = Sessao {
        usuario: cadastro.por_login(login).cloned(),
        ..Sessao::default()
    };
    assert!(ses.usuario.is_some(), "{login} nao esta no cadastro");
    let r = pede(s, &ses, r#""op":"bancos""#)?;
    Ok(r.lista()
        .unwrap()
        .iter()
        .map(|b| b.texto().unwrap().to_string())
        .collect())
}

/// **A prova do 775.** Quem tem direito so na `loja` entrava numa tela
/// vazia: o portao conferia `ler` na base VAZIA e recusava o `bancos`.
/// Defeito reposto que derruba este teste: tirar o
/// `Atividade::filtra_pela_ficha` do portao 3 (volta a recusa) ou o filtro
/// do `op_bancos` (aparece o `rh`).
#[test]
fn quem_tem_direito_numa_base_lista_so_ela() {
    let dir = dir_temp("bancos-775");
    let (s, cad) = servidor_de_duas_bases(&dir);
    let v = bancos_de(&s, &cad, "lojaadm").expect("o bancos recusou quem tem a loja");
    assert_eq!(v, vec!["loja".to_string()]);
}

/// A regra de TABELA tambem faz a base aparecer: e o caso que a ficha preve
/// (dar uma tabela a quem nao le a base), e o `SHOW DATABASES` do MySQL
/// conta o privilegio de tabela. E o `monitorar` sozinho NAO: e poder de
/// servidor, e daria a lista inteira a quem so olha o aquario.
#[test]
fn a_regra_de_tabela_mostra_a_base_e_o_monitorar_nao() {
    let dir = dir_temp("bancos-775-tab");
    let (s, cad) = servidor_de_duas_bases(&dir);
    assert_eq!(
        bancos_de(&s, &cad, "so_tabela").unwrap(),
        vec!["rh".to_string()]
    );
    assert!(bancos_de(&s, &cad, "so_aquario").unwrap().is_empty());
}

/// O comportamento VELHO, que o filtro nao pode tirar: o supervisor e quem
/// le no `"*"` continuam vendo todas as bases.
#[test]
fn o_supervisor_e_o_curinga_continuam_vendo_todas_as_bases() {
    let dir = dir_temp("bancos-775-velho");
    let (s, cad) = servidor_de_duas_bases(&dir);
    for quem in ["chefe", "curinga"] {
        let mut v = bancos_de(&s, &cad, quem).unwrap();
        v.sort();
        assert_eq!(v, vec!["loja".to_string(), "rh".to_string()], "{quem}");
    }
}

/// E o anonimo, num servidor com cadastro, continua sem listar nada: o
/// filtro tirou o portao da BASE, nao o do login.
#[test]
fn o_anonimo_continua_sem_listar_as_bases() {
    let dir = dir_temp("bancos-775-anonimo");
    let (s, _) = servidor_de_duas_bases(&dir);
    let e = pede(&s, &Sessao::default(), r#""op":"bancos""#).unwrap_err();
    assert!(e.to_string().to_lowercase().contains("login"), "{e}");
}
