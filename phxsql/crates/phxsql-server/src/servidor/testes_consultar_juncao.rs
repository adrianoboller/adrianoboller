//! A junção e a subconsulta escalar dentro do `consultar` -- o acrescimo de
//! 08/09.
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("consultar-jn-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `pedidos` (id, cliente_id, total) x `clientes` (id, nome, cidade), mais
/// a `folha` que ninguem le.
///
/// O pedido 4 aponta para o cliente 9, que nao existe: e a linha que a
/// junção interna descarta e a esquerda mantem com nulos.
fn servidor(d: &std::path::Path, cadastro: Cadastro) -> (Arc<Servidor>, Sessao) {
    servidor_com_teto(d, cadastro, 10_000)
}

fn servidor_com_teto(
    d: &std::path::Path,
    cadastro: Cadastro,
    max_linhas: u64,
) -> (Arc<Servidor>, Sessao) {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        cadastro: cadastro.clone(),
        max_linhas,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"},
                    {"nome":"cidade","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
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
        &dono,
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
        &dono,
    )
    .unwrap();
    // `produtos` tem um Decimal e um codigo que e TEXTO com cara de
    // numero; `limites` da o escalar Decimal; `vazia` nao tem linha
    // nenhuma -- e a direita vazia do pedido 237.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"produtos","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"codigo","tipo":"Str(10)"},
                    {"nome":"preco","tipo":"Decimal(10,2)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"limites","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"valor","tipo":"Decimal(10,2)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"vazia","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    // O cliente 3 nao tem pedido: e a linha da DIREITA que nao casa, a que
    // o `direito` e o `completo` mantem e o `existe` com `nao` acha.
    for (id, nome, cidade) in [
        (1, "ana", "Blumenau"),
        (2, "bia", "Itajai"),
        (3, "caio", "Joinville"),
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
    for (id, codigo, preco) in [(1, "10", "9.50"), (2, "7", "20.00"), (3, "100", "100.00")] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"produtos",
                         "linha":{{"id":{id},"codigo":"{codigo}","preco":"{preco}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"limites","linha":{"id":1,"valor":"10.00"}}"#),
        &dono,
    )
    .unwrap();
    for (id, cliente, total) in [(1, 1, 100), (2, 1, 50), (3, 2, 300), (4, 9, 7)] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"pedidos",
                         "linha":{{"id":{id},"cliente_id":{cliente},"total":{total}}}}}"#
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

fn so_le_o_que_nao_e_folha() -> Cadastro {
    Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap()
}

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

fn consultar(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.executar(
        "consultar",
        &pedido(&format!(r#"{{"database":"b",{corpo}}}"#)),
        &Sessao::default(),
    )
}

fn linhas(r: &Json) -> Vec<Json> {
    r.campo("linhas").and_then(Json::lista).unwrap().to_vec()
}

const JUNCAO: &str = r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
        "juntar":[{"de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
                   "tipo":"TIPO",
                   "em":[{"esquerda":"p.cliente_id","direita":"c.id"}]}]"#;

/// **A junção INTERNA descarta quem nao casa, e a prova mede QUANTAS
/// linhas sobraram -- nao se juntou.**
///
/// O pedido 4 aponta para o cliente 9, que nao existe. Interna: tres
/// linhas. Uma junção que ignorasse o `tipo` e sempre fizesse esquerda
/// devolveria quatro, e um teste que so perguntasse «veio o nome do
/// cliente?» passaria com esse defeito.
#[test]
fn a_juncao_interna_descarta_quem_nao_casa() {
    let d = dir("interna");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        &format!(
            r#"{},"ordem":[{{"coluna":"p.id"}}]"#,
            JUNCAO.replace("TIPO", "interno")
        ),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 3, "o pedido orfao entrou");
    let l = linhas(&r);
    // As colunas dos DOIS lados vem prefixadas.
    assert_eq!(l[0].inteiro_ou("p.id", -1), 1);
    assert_eq!(l[0].texto_ou("c.nome", ""), "ana");
    assert!(l[0].campo("id").is_none(), "sobrou nome sem prefixo: {l:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A junção ESQUERDA mantem a linha sem par, com as colunas da direita
/// NULAS.**
///
/// Quatro linhas, e a do pedido 4 com `c.nome` nulo -- e nao ausente: uma
/// coluna que some faria a projecao mudar de forma linha a linha.
#[test]
fn a_juncao_esquerda_mantem_a_linha_com_nulos() {
    let d = dir("esquerda");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        &format!(
            r#"{},"ordem":[{{"coluna":"p.id"}}]"#,
            JUNCAO.replace("TIPO", "esquerdo")
        ),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 4);
    let l = linhas(&r);
    assert_eq!(l[3].inteiro_ou("p.id", -1), 4);
    assert_eq!(
        l[3].campo("c.nome"),
        Some(&Json::Nulo),
        "a coluna da direita sumiu em vez de vir nula: {l:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Nome sem prefixo resolve quando e UNICO, e recusa NOMEANDO os dois
/// candidatos quando nao e.
///
/// `id` existe nos dois lados; `cidade` so num deles. Escolher um dos dois
/// `id` calado responderia sobre a coluna errada.
#[test]
fn nome_ambiguo_recusa_e_nome_unico_resolve() {
    let d = dir("ambiguo");
    let (s, _) = servidor(&d, Cadastro::default());
    let base = JUNCAO.replace("TIPO", "interno");

    let e = consultar(&s, &format!(r#"{base},"expressao":"id > 1""#)).unwrap_err();
    let t = e.to_string();
    assert!(t.contains("ambiguo"), "{t}");
    assert!(
        t.contains("p.id") && t.contains("c.id"),
        "a recusa nao diz os dois: {t}"
    );

    // `cidade` so existe de um lado: resolve sem prefixo.
    let r = consultar(
        &s,
        &format!(r#"{base},"expressao":"cidade = 'Itajai'","colunas":["p.id","nome"]"#),
    )
    .unwrap();
    let l = linhas(&r);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].inteiro_ou("p.id", -1), 3);
    assert_eq!(l[0].texto_ou("nome", ""), "bia");
    let _ = std::fs::remove_dir_all(&d);
}

/// A projecao aceita `{"coluna","apelido"}`, e a chave da saida e o
/// apelido.
#[test]
fn a_projecao_aceita_apelido() {
    let d = dir("apelido");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        &format!(
            r#"{},"ordem":[{{"coluna":"p.id"}}],
                   "colunas":["p.id",{{"coluna":"c.nome","apelido":"cliente"}}]"#,
            JUNCAO.replace("TIPO", "interno")
        ),
    )
    .unwrap();
    let l = linhas(&r);
    assert_eq!(l[0].texto_ou("cliente", ""), "ana");
    assert!(l[0].campo("c.nome").is_none(), "saiu com o nome interno");
    let _ = std::fs::remove_dir_all(&d);
}

/// Junção de tipo desconhecido recusa nomeando; sem par (fora do
/// `cruzado`) recusa como produto cartesiano disfarcado; e o `cruzado`
/// COM `em` recusa, porque produto nao casa por igualdade.
#[test]
fn as_recusas_de_tipo_e_de_par_nomeiam() {
    let d = dir("tipos");
    let (s, _) = servidor(&d, Cadastro::default());
    let e = consultar(&s, &JUNCAO.replace("TIPO", "lateral")).unwrap_err();
    let t = e.to_string();
    assert!(t.contains("lateral") && t.contains("cruzado"), "{t}");
    // Junção sem par nenhum e produto cartesiano disfarcado.
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
               "juntar":[{"de":{"op":"varrer","tabela":"clientes"},"apelido":"c"}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("cartesiano"), "{e}");
    // E o cruzado com `em` se contradiz.
    let e = consultar(&s, &JUNCAO.replace("TIPO", "cruzado")).unwrap_err();
    assert!(e.to_string().contains("nao aceita \"em\""), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A subconsulta ESCALAR roda uma vez e vira coluna da expressao -- e
/// duas linhas RECUSAM nomeando.
///
/// Escolher a primeira faria a resposta depender da ordem em que o motor
/// devolveu as linhas, e «depende da ordem» num numero e o defeito que
/// nao se acha.
#[test]
fn o_escalar_vira_coluna_e_duas_linhas_recusam() {
    let d = dir("escalar");
    let (s, _) = servidor(&d, Cadastro::default());
    // A media dos totais e (100+50+300+7)/4 = 114,25.
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},
               "escalar":[{"nome":"media","campo":"media_total",
                           "de":{"op":"agrupar","tabela":"pedidos",
                                 "agregados":[{"funcao":"media","coluna":"total"}]}}],
               "expressao":"total > media","ordem":[{"coluna":"id"}]"#,
    )
    .unwrap();
    let l = linhas(&r);
    assert_eq!(l.len(), 1, "so o pedido 3 passa da media");
    assert_eq!(l[0].inteiro_ou("id", -1), 3);
    // E a coluna do escalar NAO sai na resposta sem ser pedida.
    assert!(l[0].campo("media").is_none(), "o escalar vazou: {l:?}");

    // Pedida pelo nome, ela sai.
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos","max":1},
               "escalar":[{"nome":"media","campo":"media_total",
                           "de":{"op":"agrupar","tabela":"pedidos",
                                 "agregados":[{"funcao":"media","coluna":"total"}]}}],
               "colunas":["id","media"]"#,
    )
    .unwrap();
    assert!(linhas(&r)[0].campo("media").is_some());

    // Duas linhas recusam, dizendo quantas vieram e o que fazer.
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},
               "escalar":[{"nome":"x","campo":"id",
                           "de":{"op":"varrer","tabela":"clientes"}}]"#,
    )
    .unwrap_err();
    let t = e.to_string();
    assert!(t.contains('3') && t.contains("exatamente uma"), "{t}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A PORTA DOS FUNDOS, agora com tres entradas: `de`, `juntar[].de` e
/// `escalar[].de`.**
///
/// Ana nao le a folha. Cada um dos tres caminhos sai pelo
/// `executar_derivado`, entao os tres recusam com o mesmo erro. Este e o
/// teste que importa do acrescimo inteiro: se ele passar a falhar, alguem
/// deu ao `consultar` um caminho proprio ate o dado.
///
/// PROVA REAL: trocando `executar_derivado` por `executar` em
/// `linhas_do_sub_pedido`, as tres recusas viram `Ok` e este teste reprova
/// nas tres -- e o controle (a tabela permitida, na mesma corrida)
/// continua passando, para uma quebra por outro motivo nao passar por
/// engano.
#[test]
fn o_consultar_nao_e_a_porta_dos_fundos_pela_juncao_nem_pelo_escalar() {
    let d = dir("porta");
    let (s, ses) = servidor(&d, so_le_o_que_nao_e_folha());

    // Controle: a junção entre duas tabelas permitidas passa.
    let ok = pede(
        &s,
        &ses,
        &format!(
            r#""op":"consultar","database":"b",{}"#,
            JUNCAO.replace("TIPO", "interno")
        ),
    )
    .expect("as tabelas permitidas tinham de passar");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 3);

    // (1) a folha como `de` da junção.
    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
               "juntar":[{"de":{"op":"varrer","tabela":"folha"},"apelido":"f",
                          "em":[{"esquerda":"p.id","direita":"f.id"}]}]"#,
    )
    .expect_err("a junção leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    // (2) a folha como `de` de um escalar.
    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"varrer","tabela":"pedidos"},
               "escalar":[{"nome":"x","campo":"nome",
                           "de":{"op":"varrer","tabela":"folha"}}]"#,
    )
    .expect_err("o escalar leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// **A UNIAO nao e a porta dos fundos para o lado B.**
///
/// Ela mora ao lado do teste do `consultar` porque e a MESMA lei: o campo
/// `"tabela"` que o portao geral le nao existe num pedido de uniao, entao
/// o `op_unir` confere tabela por tabela -- e a traducao do `UNION` nao
/// acrescenta conferencia nenhuma, ela desemboca naquele pedido.
///
/// O que prova e o lado B NEGADO: o lado A passa sozinho, entao um portao
/// que so olhasse a primeira tabela nao acusaria nada.
#[test]
fn a_uniao_pela_op_sql_nao_e_a_porta_dos_fundos_para_o_lado_b() {
    let d = dir("porta-uniao");
    let (s, ses) = servidor(&d, so_le_o_que_nao_e_folha());

    // Controle: a uniao de duas tabelas permitidas passa.
    let ok = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b",
               "texto":"SELECT * FROM clientes UNION ALL SELECT * FROM clientes""#,
    )
    .expect("a tabela permitida tinha de passar");
    assert_eq!(ok.texto_ou("op", ""), "unir", "{ok:?}");

    // A folha como lado B -- o lado que o portao geral nao enxerga.
    let e = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b",
               "texto":"SELECT * FROM clientes UNION SELECT * FROM folha""#,
    )
    .expect_err("a uniao leu a tabela negada pelo lado B");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    // E como lado A tambem, que e o caso que o portao geral pegaria --
    // aqui ele NAO pega, porque nao ha campo `"tabela"` no pedido.
    let e = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b",
               "texto":"SELECT * FROM folha UNION SELECT * FROM clientes""#,
    )
    .expect_err("a uniao leu a tabela negada pelo lado A");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// Apelido repetido recusa: as colunas dos dois lados se sobreporiam, e o
/// nome qualificado deixaria de qualificar.
#[test]
fn apelido_repetido_na_juncao_recusa() {
    let d = dir("apelido-rep");
    let (s, _) = servidor(&d, Cadastro::default());
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
               "juntar":[{"de":{"op":"varrer","tabela":"clientes"},"apelido":"p",
                          "em":[{"esquerda":"p.cliente_id","direita":"p.id"}]}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("\"p\""), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Sem apelido escrito, o apelido e o NOME DA TABELA -- e um sub-pedido
/// que nao nomeia tabela recusa pedindo um.
#[test]
fn o_apelido_padrao_e_o_nome_da_tabela() {
    let d = dir("padrao");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},
               "juntar":[{"de":{"op":"varrer","tabela":"clientes"},
                          "em":[{"esquerda":"pedidos.cliente_id",
                                 "direita":"clientes.id"}]}],
               "ordem":[{"coluna":"pedidos.id"}]"#,
    )
    .unwrap();
    assert_eq!(linhas(&r)[0].texto_ou("clientes.nome", ""), "ana");

    let e = consultar(
        &s,
        r#""de":{"op":"consultar","de":{"op":"varrer","tabela":"pedidos"}},
               "juntar":[{"de":{"op":"varrer","tabela":"clientes"},
                          "em":[{"esquerda":"id","direita":"id"}]}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("apelido"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
/// **A junção ESQUERDA com a DIREITA VAZIA traz as colunas da direita
/// PRESENTES e NULAS** (pedido 237, defeito 1).
///
/// Com o defeito reposto (o modelo da direita saindo de `direita.first()`),
/// as colunas `v.id` e `v.nome` SOMEM da linha: a forma da linha muda entre
/// o caso que casou e o que nao casou, e quem le por posicao quebra na
/// primeira orfa. O cabecalho `colunas` tem de nomea-las tambem, porque e
/// por ele que um `consultar` aninhado sabe a forma sem esperar linha.
#[test]
fn a_juncao_esquerda_com_a_direita_vazia_traz_as_colunas_nulas() {
    let d = dir("direita-vazia");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
               "juntar":[{"de":{"op":"varrer","tabela":"vazia"},"apelido":"v",
                          "tipo":"esquerdo",
                          "em":[{"esquerda":"p.cliente_id","direita":"v.id"}]}],
               "ordem":[{"coluna":"p.id"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 4);
    for l in linhas(&r) {
        assert_eq!(
            l.campo("v.nome"),
            Some(&Json::Nulo),
            "a coluna da direita vazia sumiu em vez de vir nula: {l:?}"
        );
        assert_eq!(l.campo("v.id"), Some(&Json::Nulo), "{l:?}");
    }
    let nomes: Vec<String> = r
        .campo("colunas")
        .and_then(Json::lista)
        .expect("a resposta traz o modelo em \"colunas\"")
        .iter()
        .map(|c| c.texto_ou("nome", "").to_string())
        .collect();
    assert!(nomes.contains(&"v.nome".to_string()), "{nomes:?}");
    assert!(nomes.contains(&"p.total".to_string()), "{nomes:?}");
    let _ = std::fs::remove_dir_all(&d);
}

fn ids(r: &Json) -> Vec<i64> {
    linhas(r).iter().map(|l| l.inteiro_ou("id", -1)).collect()
}

/// A FORMA de cada linha: os nomes, na ordem. Toda linha de uma resposta
/// tem de ter a mesma.
fn formas(r: &Json) -> Vec<Vec<String>> {
    linhas(r)
        .iter()
        .map(|l| l.chaves().into_iter().map(String::from).collect())
        .collect()
}

fn cabecalho(r: &Json) -> Vec<(String, String)> {
    r.campo("colunas")
        .and_then(Json::lista)
        .expect("a resposta traz \"colunas\"")
        .iter()
        .map(|c| {
            (
                c.texto_ou("nome", "").to_string(),
                c.texto_ou("tipo", "").to_string(),
            )
        })
        .collect()
}

/// **`preco > media` com `preco` Decimal `9.50` e `media` `10.00` da
/// FALSO** (pedido 237, defeito 2).
///
/// Com o defeito reposto (a celula convertida pelo FORMATO do JSON e nao
/// pelo tipo do modelo), `"9.50" > "10.00"` compara como texto e da
/// verdadeiro -- e o produto de 9,50 passa por um filtro que pede acima
/// de 10. O teste mede QUANTAS linhas sobraram, e nao se a consulta
/// respondeu.
#[test]
fn o_decimal_da_expressao_compara_como_numero() {
    let d = dir("decimal");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"produtos"},
               "escalar":[{"nome":"media","campo":"valor",
                           "de":{"op":"varrer","tabela":"limites"}}],
               "expressao":"preco > media","ordem":[{"coluna":"id"}]"#,
    )
    .unwrap();
    let l = linhas(&r);
    let ids: Vec<i64> = l.iter().map(|x| x.inteiro_ou("id", -1)).collect();
    assert_eq!(ids, vec![2, 3], "9,50 passou de 10,00: {l:?}");

    // O literal tambem: `preco > 10` compara Decimal com numero.
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"produtos"},
               "expressao":"preco > 10","ordem":[{"coluna":"id"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2);

    // E a coluna Str que CONTEM "10" continua texto: rotulo se estiliza,
    // dado nunca. `codigo = '10'` casa; `codigo > 9` recusa por tipo.
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"produtos"},"expressao":"codigo = '10'""#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1);
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"produtos"},"expressao":"codigo > 9""#,
    )
    .expect_err("texto contra numero tinha de recusar");
    assert!(e.to_string().contains("nao da para comparar"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **`direito` e `completo`, medidos em QUANTAS linhas e em que FORMA.**
///
/// `direito`: as tres casadas mais o caio, que nao tem pedido -- e a linha
/// dele sai com `p.*` NULO na frente, na mesma forma das outras. O pedido
/// 4 (orfao da esquerda) NAO entra. `completo`: as tres, o pedido 4 e o
/// caio -- cinco, todas com a mesma forma.
#[test]
fn as_juncoes_direita_e_completa_mantem_o_lado_certo() {
    let d = dir("direito-completo");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        &format!(
            r#"{},"ordem":[{{"coluna":"c.id"}},{{"coluna":"p.id"}}]"#,
            JUNCAO.replace("TIPO", "direito")
        ),
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 4, "{}", r.escrever());
    let l = linhas(&r);
    let caio = l
        .iter()
        .find(|x| x.texto_ou("c.nome", "") == "caio")
        .expect("o cliente sem pedido sumiu do direito");
    assert_eq!(caio.campo("p.id"), Some(&Json::Nulo), "{caio:?}");
    assert_eq!(caio.campo("p.total"), Some(&Json::Nulo), "{caio:?}");
    assert!(
        l.iter().all(|x| x.inteiro_ou("p.id", -1) != 4),
        "o orfao da esquerda entrou no direito: {l:?}"
    );
    let f = formas(&r);
    assert!(
        f.iter().all(|x| *x == f[0]),
        "a forma mudou entre linhas: {f:?}"
    );
    assert_eq!(
        f[0][0], "p.rowid",
        "a esquerda tem de vir na frente: {:?}",
        f[0]
    );

    let r = consultar(&s, &JUNCAO.replace("TIPO", "completo")).unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 5, "{}", r.escrever());
    let l = linhas(&r);
    assert!(
        l.iter()
            .any(|x| x.inteiro_ou("p.id", -1) == 4 && x.campo("c.nome") == Some(&Json::Nulo)),
        "o orfao da esquerda sumiu do completo: {l:?}"
    );
    assert!(
        l.iter()
            .any(|x| x.texto_ou("c.nome", "") == "caio" && x.campo("p.id") == Some(&Json::Nulo)),
        "o orfao da direita sumiu do completo: {l:?}"
    );
    let f = formas(&r);
    assert!(f.iter().all(|x| *x == f[0]), "{f:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **`cruzado` e o produto, e o teto e conferido ANTES de materializar.**
///
/// 4 pedidos x 3 clientes = 12. Com `max_linhas` 10, a recusa nomeia os
/// dois tamanhos e diz que veio antes -- e o teste confere que ela vem
/// como `LimiteExcedido`, e nao como um produto que estourou depois.
#[test]
fn a_juncao_cruzada_e_o_produto_e_o_teto_vem_antes() {
    let d = dir("cruzado");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
               "juntar":[{"de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
                          "tipo":"cruzado"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 12);
    let f = formas(&r);
    assert!(f.iter().all(|x| *x == f[0]), "{f:?}");
    let _ = std::fs::remove_dir_all(&d);

    let d = dir("cruzado-teto");
    let (s, _) = servidor_com_teto(&d, Cadastro::default(), 10);
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
               "juntar":[{"de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
                          "tipo":"cruzado"}]"#,
    )
    .expect_err("12 linhas com teto 10 tinham de recusar");
    let t = e.to_string();
    assert!(
        t.contains("4 x 3") && t.contains("12"),
        "nao nomeia os tamanhos: {t}"
    );
    assert!(t.contains("ANTES"), "{t}");
    assert!(matches!(e, PhxError::LimiteExcedido(_)), "{e:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **As outras quatro junções PARAM no teto, na linha `teto + 1`, sem
/// materializar o resto** -- a regra que ja valia para o `cruzado`.
///
/// Antes o teto era conferido sobre a lista pronta: 1000 x 1000 com a
/// mesma chave materializava um milhao de linhas (+561 MiB, medidos pelo
/// soquete) para recusar contra um teto de mil. Aqui a prova e de
/// CONTRATO -- a memoria se mede na bancada --: no teto exato passa, uma
/// linha acima recusa como `LimiteExcedido` dizendo que parou, e a
/// `esquerda` (que tem uma linha a mais, a orfa) cai onde a `interna`
/// passa.
#[test]
fn as_juncoes_por_par_param_no_teto_sem_materializar_o_resto() {
    // `pedidos` com ela mesma, por `cliente_id`: 4 linhas de cada lado
    // viram 6 (ana tem dois pedidos: 2 x 2, mais 1 x 1 de bia e 1 x 1 do
    // orfao). Os quatro tipos dao 6, porque toda linha casa consigo.
    // O teto tem de ficar ACIMA das entradas (4), senao e o `varrer` do
    // sub-pedido que corta -- foi o primeiro desenho deste teste, e ele
    // media o teto errado.
    const AUTO: &str = r#""de":{"op":"varrer","tabela":"pedidos"},"apelido":"p",
            "juntar":[{"de":{"op":"varrer","tabela":"pedidos"},"apelido":"q",
                       "tipo":"TIPO",
                       "em":[{"esquerda":"p.cliente_id","direita":"q.cliente_id"}]}]"#;
    let d = dir("teto-exato");
    let (s, _) = servidor_com_teto(&d, Cadastro::default(), 6);
    for tipo in ["interno", "esquerdo", "direito", "completo"] {
        let r = consultar(&s, &AUTO.replace("TIPO", tipo))
            .unwrap_or_else(|e| panic!("{tipo} no teto exato: {e:?}"));
        assert_eq!(r.inteiro_ou("devolvidas", -1), 6, "{tipo}");
    }
    let _ = std::fs::remove_dir_all(&d);

    let d = dir("teto-uma-a-menos");
    let (s, _) = servidor_com_teto(&d, Cadastro::default(), 5);
    for tipo in ["interno", "esquerdo", "direito", "completo"] {
        let e = consultar(&s, &AUTO.replace("TIPO", tipo))
            .expect_err("6 linhas com teto 5 tinham de recusar");
        let t = e.to_string();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "{tipo}: {e:?}");
        assert!(
            t.contains("mais de 5") && t.contains("linha 6"),
            "{tipo}: {t}"
        );
        assert!(t.contains("sem materializar"), "{tipo}: {t}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// **`existe`: quem tem par fica UMA vez; `nao` inverte; sem `em` recusa;
/// e sem `existe` nada muda.**
///
/// Ana tem dois pedidos e sai uma vez -- e a diferenca entre `EXISTS` e
/// `JOIN`, e `ids` a mede. O caio nao tem pedido: e o unico do `nao`.
#[test]
fn o_existe_filtra_quem_tem_par_e_o_nao_quem_nao_tem() {
    let d = dir("existe");
    let (s, _) = servidor(&d, Cadastro::default());
    const EXISTE: &str = r#""de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
            "existe":[{"de":{"op":"varrer","tabela":"pedidos"},"apelido":"x",
                       "em":[{"esquerda":"c.id","direita":"x.cliente_id"}],
                       "nao":NAO}],
            "ordem":[{"coluna":"id"}]"#;
    let r = consultar(&s, &EXISTE.replace("NAO", "false")).unwrap();
    assert_eq!(ids(&r), vec![1, 2], "{}", r.escrever());
    // Nao acrescenta coluna nenhuma: a forma e a do `varrer` de clientes.
    let l = linhas(&r);
    assert!(
        l[0].campo("x.id").is_none() && l[0].campo("cliente_id").is_none(),
        "{l:?}"
    );
    assert_eq!(l[0].texto_ou("nome", ""), "ana");

    let r = consultar(&s, &EXISTE.replace("NAO", "true")).unwrap();
    assert_eq!(ids(&r), vec![3], "{}", r.escrever());

    // Sem `em` recusa nomeando o campo.
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "existe":[{"de":{"op":"varrer","tabela":"pedidos"}}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("\"em\""), "{e}");

    // Com junção na frente, o lado de fora ja esta prefixado e `c.id`
    // resolve contra o modelo composto: os pedidos da ana (cliente 1),
    // porque `limites` so tem o id 1.
    let r = consultar(
        &s,
        &format!(
            r#"{},"existe":[{{"de":{{"op":"varrer","tabela":"limites"}},"apelido":"l",
                                "em":[{{"esquerda":"c.id","direita":"l.id"}}]}}],
                   "ordem":[{{"coluna":"p.id"}}]"#,
            JUNCAO.replace("TIPO", "interno")
        ),
    )
    .unwrap();
    let l = linhas(&r);
    let ps: Vec<i64> = l.iter().map(|x| x.inteiro_ou("p.id", -1)).collect();
    assert_eq!(ps, vec![1, 2], "{l:?}");

    // O comportamento VELHO: sem `existe`, os tres clientes, como sempre.
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},"ordem":[{"coluna":"id"}]"#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![1, 2, 3]);
    let _ = std::fs::remove_dir_all(&d);
}

/// **Pedido 240, o caminho INTEIRO pela op `sql`: `EXISTS` correlacionado
/// por apelido de fora.** A forma-modelo do `docs/SQL.md` §10
/// (`FROM clientes c ... WHERE x.cliente_id = c.id`) recusava, porque o
/// tradutor nao emitia o apelido do `de` sem junção e o `consultar` nao
/// sabia que `c.id` era a coluna `id` da linha de fora.
///
/// PROVA REAL nos dois sentidos: com o defeito reposto (o tradutor deixa de
/// emitir `apelido` sem junção), a op `sql` recusa nomeando «o lado de fora
/// do existe 0 pede "c.id"...»; com o conserto, devolve ana e bia (que tem
/// pedido) e nao o caio (que nao tem). O filtro de dentro (`x.total > 200`)
/// perde o `x.` (parte 3) e resolve na tabela crua; a projecao e a ordem
/// qualificadas (`c.nome`, `c.id`, parte 2) descartam o `c.` no servidor.
#[test]
fn exists_por_apelido_de_fora_pela_op_sql() {
    let d = dir("existe-apelido-fora");
    let (s, ses) = servidor(&d, Cadastro::default());
    let roda = |texto: &str| {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"sql","database":"b","texto":{}"#,
                Json::texto_de(texto).escrever()
            ),
        )
    };

    // A forma-modelo do §10 -- `SELECT *`, ordem por coluna nua.
    let r = roda(
        "SELECT * FROM clientes c WHERE EXISTS \
             (SELECT 1 FROM pedidos AS x WHERE x.cliente_id = c.id) ORDER BY id",
    )
    .expect("a forma-modelo do §10 tinha de devolver linhas, nao recusar");
    assert_eq!(ids(&r), vec![1, 2], "ana e bia tem pedido; caio nao");

    // NOT EXISTS: so o caio, que nao tem pedido.
    let r = roda(
        "SELECT * FROM clientes c WHERE NOT EXISTS \
             (SELECT 1 FROM pedidos AS x WHERE x.cliente_id = c.id) ORDER BY id",
    )
    .unwrap();
    assert_eq!(ids(&r), vec![3]);

    // Parte 3: o filtro de dentro `x.total > 200` perde o `x.` e resolve na
    // tabela crua. So a bia (pedido de 300) casa; a ana (100 e 50) nao.
    let r = roda(
        "SELECT * FROM clientes c WHERE EXISTS \
             (SELECT 1 FROM pedidos AS x WHERE x.cliente_id = c.id AND x.total > 200) \
             ORDER BY id",
    )
    .expect("o filtro de dentro com x. tinha de resolver, nao recusar");
    assert_eq!(ids(&r), vec![2], "so a bia tem pedido acima de 200");

    // Parte 2: projecao e ordem qualificadas pelo apelido de fora.
    let r = roda(
        "SELECT c.nome FROM clientes c WHERE EXISTS \
             (SELECT 1 FROM pedidos AS x WHERE x.cliente_id = c.id) ORDER BY c.id",
    )
    .expect("projecao e ordem por c. tinham de resolver, nao recusar");
    let nomes: Vec<String> = linhas(&r)
        .iter()
        .map(|l| l.texto_ou("c.nome", "").to_string())
        .collect();
    assert_eq!(nomes, vec!["ana".to_string(), "bia".to_string()]);

    let _ = std::fs::remove_dir_all(&d);
}

/// **A PORTA DOS FUNDOS numero cinco: `existe[].de`.**
///
/// Ana nao le a folha. O `existe` nao devolve coluna nenhuma da tabela de
/// dentro -- e mesmo assim tem de recusar, porque «quais clientes tem
/// linha na folha?» ja e uma pergunta sobre a folha. O controle na mesma
/// corrida: o mesmo `existe` sobre a tabela permitida responde.
///
/// PROVA REAL: trocando `executar_derivado` por `executar` em
/// `linhas_do_sub_pedido`, a recusa vira `Ok` e este teste reprova.
#[test]
fn o_existe_nao_e_a_porta_dos_fundos() {
    let d = dir("existe-porta");
    let (s, ses) = servidor(&d, so_le_o_que_nao_e_folha());
    let ok = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"varrer","tabela":"clientes"},
               "existe":[{"de":{"op":"varrer","tabela":"pedidos"},"apelido":"x",
                          "em":[{"esquerda":"id","direita":"x.cliente_id"}]}]"#,
    )
    .expect("a tabela permitida tinha de passar");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 2);

    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"varrer","tabela":"clientes"},
               "existe":[{"de":{"op":"varrer","tabela":"folha"},"apelido":"f",
                          "em":[{"esquerda":"id","direita":"f.id"}]}]"#,
    )
    .expect_err("o existe leu a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **`COUNT(coluna)` conta valores NAO NULOS; sem coluna conta linhas** --
/// e o cabecalho diz o tipo de cada agregado, MEDIDO: contagem `UInt8`,
/// soma e media de inteiro `Real8` (e nao o tipo da coluna, como o
/// contrato supunha), soma e media de Decimal `Decimal { 38, escala }`.
#[test]
fn a_contagem_por_coluna_ignora_o_nulo_e_o_cabecalho_diz_o_tipo() {
    let d = dir("contagem");
    let (s, _) = servidor(&d, Cadastro::default());
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"notas","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"valor","tipo":"Int4"},
                    {"nome":"preco","tipo":"Decimal(10,2)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for linha in [
        r#"{"id":1,"valor":5,"preco":"1.50"}"#,
        r#"{"id":2,"valor":null,"preco":"2.50"}"#,
        r#"{"id":3,"valor":7,"preco":null}"#,
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"notas","linha":{linha}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let r = s
        .executar(
            "agrupar",
            &pedido(
                r#"{"database":"b","tabela":"notas","agregados":[
                        {"funcao":"contagem"},
                        {"funcao":"contagem","coluna":"valor","apelido":"com_valor"},
                        {"funcao":"soma","coluna":"valor"},
                        {"funcao":"media","coluna":"valor"},
                        {"funcao":"soma","coluna":"preco"},
                        {"funcao":"media","coluna":"preco"}]}"#,
            ),
            &dono,
        )
        .unwrap();
    let l = linhas(&r);
    assert_eq!(l[0].inteiro_ou("contagem", -1), 3, "{}", r.escrever());
    assert_eq!(
        l[0].inteiro_ou("com_valor", -1),
        2,
        "contou o nulo: {}",
        r.escrever()
    );
    assert_eq!(l[0].inteiro_ou("soma_valor", -1), 12);
    assert_eq!(l[0].inteiro_ou("media_valor", -1), 6);
    assert_eq!(l[0].texto_ou("soma_preco", ""), "4.00");
    assert_eq!(l[0].texto_ou("media_preco", ""), "2.00");
    let tipos = cabecalho(&r);
    let tipo = |n: &str| tipos.iter().find(|(m, _)| m == n).map(|(_, t)| t.as_str());
    assert_eq!(tipo("contagem"), Some("UInt8"));
    assert_eq!(tipo("com_valor"), Some("UInt8"));
    assert_eq!(tipo("soma_valor"), Some("Real8"));
    assert_eq!(tipo("media_valor"), Some("Real8"));
    assert_eq!(
        tipo("soma_preco"),
        Some("Decimal { precisao: 38, escala: 2 }")
    );
    assert_eq!(
        tipo("media_preco"),
        Some("Decimal { precisao: 38, escala: 2 }")
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **A ordem aceita nome QUALIFICADO, resolve o unico sem prefixo, recusa
/// o ambiguo nomeando os dois -- e ordena `Decimal` como NUMERO.**
///
/// Como texto, `"100.00"` viria antes de `"9.50"`. E a mesma raiz do
/// defeito 2 do pedido 237, no `ordem`.
#[test]
fn a_ordem_aceita_nome_qualificado_e_ordena_decimal_como_numero() {
    let d = dir("ordem");
    let (s, _) = servidor(&d, Cadastro::default());
    let base = JUNCAO.replace("TIPO", "interno");
    let e = consultar(&s, &format!(r#"{base},"ordem":[{{"coluna":"id"}}]"#)).unwrap_err();
    let t = e.to_string();
    assert!(t.contains("p.id") && t.contains("c.id"), "{t}");

    let r = consultar(
        &s,
        &format!(r#"{base},"ordem":[{{"coluna":"total","desc":true}}]"#),
    )
    .unwrap();
    assert_eq!(linhas(&r)[0].inteiro_ou("p.total", -1), 300);

    let r = consultar(
        &s,
        &format!(r#"{base},"ordem":[{{"coluna":"c.nome","desc":true}},"p.id"]"#),
    )
    .unwrap();
    assert_eq!(linhas(&r)[0].texto_ou("c.nome", ""), "bia");

    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"produtos"},"ordem":[{"coluna":"preco"}]"#,
    )
    .unwrap();
    let precos: Vec<String> = linhas(&r)
        .iter()
        .map(|l| l.texto_ou("preco", "").to_string())
        .collect();
    assert_eq!(precos, vec!["9.50", "20.00", "100.00"]);
    let _ = std::fs::remove_dir_all(&d);
}

/// **O cabecalho `colunas` diz a forma -- inclusive sobre o VAZIO -- e um
/// `consultar` aninhado o recebe sem perder o tipo.**
///
/// O de dentro projeta `preco` como `valor`; o de fora compara `valor > 10`
/// e so acerta se o tipo Decimal atravessou o cabecalho. Sobre a tabela
/// vazia, a forma existe e a coluna que nao existe recusa nomeando as que
/// ha -- antes do modelo, o vazio nao tinha contra o que resolver.
#[test]
fn o_cabecalho_colunas_faz_a_ida_e_volta_no_consultar_aninhado() {
    let d = dir("cabecalho");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"consultar","de":{"op":"varrer","tabela":"produtos"},
                     "colunas":["id",{"coluna":"preco","apelido":"valor"}]},
               "expressao":"valor > 10","ordem":[{"coluna":"id"}]"#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![2, 3], "{}", r.escrever());
    assert_eq!(
        cabecalho(&r),
        vec![
            ("id".to_string(), "Int4".to_string()),
            (
                "valor".to_string(),
                "Decimal { precisao: 10, escala: 2 }".to_string()
            ),
        ]
    );

    let r = consultar(&s, r#""de":{"op":"varrer","tabela":"vazia"}"#).unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 0);
    let nomes: Vec<String> = cabecalho(&r).into_iter().map(|(n, _)| n).collect();
    assert_eq!(nomes[0], "rowid");
    assert!(nomes.contains(&"nome".to_string()), "{nomes:?}");

    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"vazia"},"colunas":["nome_errado"]"#,
    )
    .unwrap_err();
    let t = e.to_string();
    assert!(
        t.contains("nome_errado") && t.contains("rowid, id, nome"),
        "{t}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
