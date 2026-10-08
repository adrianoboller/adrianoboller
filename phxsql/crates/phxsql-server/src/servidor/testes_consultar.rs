//! A op `consultar` -- a composicao, e o portao que ela NAO abre.
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("consultar-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Uma base `b` com `clientes`, `folha` e `pagos`.
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
    for tab in ["clientes", "folha", "pagos"] {
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
        (4, "duda", "Joinville"),
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
    // `pagos` tem so os ids 2 e 3 -- e o conjunto do `IN`.
    for id in [2, 3] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"pagos","linha":{{"id":{id},"nome":"x"}}}}"#
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

fn so_le_clientes_e_pagos() -> Cadastro {
    Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap()
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade.
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

fn ids(r: &Json) -> Vec<i64> {
    r.campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect()
}

/// **O TESTE QUE MAIS IMPORTA DO ITEM INTEIRO.**
///
/// Ana le `clientes` e nao le `folha`. Pedir a folha COMO SUB-PEDIDO nao
/// muda isso -- nem no `de`, nem dentro de um `em`. Se este teste passar a
/// falhar, alguem trocou o `executar_derivado` por uma leitura direta da
/// tabela, e o `consultar` virou a porta dos fundos que o `juntar` e o
/// `unir` ja foram uma vez.
///
/// PROVA REAL: trocando `self.executar_derivado(&op, &pedido, sessao)` por
/// `self.executar(&op, &pedido, sessao)` em `linhas_do_sub_pedido`, as
/// duas recusas viram `Ok` com a linha da folha dentro, e este teste
/// REPROVA nas duas -- e ele confere o CONTEUDO da resposta permitida na
/// mesma corrida, para uma quebra por outro motivo nao passar por engano.
/// **`em` com `campo` que o sub-pedido NAO devolve recusa nomeando -- e
/// nao responde zero linhas com `ok: true`.** O `escalar` e o `existe`
/// ja resolviam o campo contra o modelo do sub-pedido; o `em` era o irmao
/// que resolvia linha a linha com `filter_map`, e a ausencia virava um
/// conjunto vazio: `id IN (SELECT zzz FROM pagos)` dava «nenhum»
/// calado, que e a resposta errada com cara de certa.
#[test]
fn o_em_com_campo_que_o_sub_pedido_nao_devolve_recusa_nomeando() {
    let d = dir("em-campo");
    let (s, _) = servidor(&d, Cadastro::default());
    let e = s
        .executar(
            "consultar",
            &pedido(
                r#"{"database":"b","de":{"op":"varrer","tabela":"clientes"},
                        "em":[{"coluna":"id","de":{"op":"varrer","tabela":"pagos"},
                               "campo":"zzz"}]}"#,
            ),
            &Sessao::default(),
        )
        .expect_err("o campo inexistente deu zero linhas calado");
    let t = e.to_string();
    assert!(t.contains("zzz") && t.contains("\"em\""), "{t}");
    assert!(t.contains("nome"), "nao lista as colunas que ha: {t}");
    // E o `campo` certo continua filtrando: pagos tem os ids 2 e 3.
    let r = s
        .executar(
            "consultar",
            &pedido(
                r#"{"database":"b","de":{"op":"varrer","tabela":"clientes"},
                        "em":[{"coluna":"id","de":{"op":"varrer","tabela":"pagos"},
                               "campo":"id"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{r:?}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn o_consultar_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
    let d = dir("porta");
    let (s, ses) = servidor(&d, so_le_clientes_e_pagos());

    // O controle: a tabela permitida passa, e traz o que tem de trazer.
    let ok = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"varrer","tabela":"clientes"}"#,
    )
    .expect("a tabela permitida tinha de passar");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 4);

    // O `de` pedindo a tabela negada.
    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"folha"}"#,
    )
    .expect_err("o consultar leu a tabela negada pelo \"de\"");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    // E o `em` pedindo a tabela negada por dentro -- o disfarce mais
    // facil de nao ver, porque a tabela mora dois niveis abaixo.
    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"varrer","tabela":"clientes"},
               "em":[{"coluna":"id","de":{"op":"varrer","tabela":"folha"},"campo":"id"}]"#,
    )
    .expect_err("o consultar leu a tabela negada pelo \"em\"");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// O `em` e o `IN (SELECT …)`: so quem esta no conjunto sobra.
#[test]
fn o_em_e_o_in_de_subconsulta() {
    let d = dir("em");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "em":[{"coluna":"id","de":{"op":"varrer","tabela":"pagos"},"campo":"id"}]"#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![2, 3]);
    assert_eq!(r.inteiro_ou("achadas", -1), 2);
    let _ = std::fs::remove_dir_all(&d);
}

/// A expressao filtra a linha composta, e `NULL` exclui.
#[test]
fn a_expressao_filtra_a_linha_composta() {
    let d = dir("expr");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "expressao":"cidade = 'Blumenau' AND id > 1""#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![3]);
    let _ = std::fs::remove_dir_all(&d);
}

/// `row_number` numera por particao -- e nao reordena a resposta.
///
/// A ordem da saida e a do `ordem`, e nao a da janela: numerar
/// reordenando faria a janela deixar de ser uma coluna para virar um
/// efeito colateral.
#[test]
fn a_janela_numera_por_particao() {
    let d = dir("janela");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "janela":[{"funcao":"row_number","particao":["cidade"],
                          "ordem":[{"coluna":"id","desc":true}],"apelido":"n"}],
               "ordem":[{"coluna":"id"}]"#,
    )
    .unwrap();
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(ids(&r), vec![1, 2, 3, 4], "a janela reordenou a resposta");
    // Em Blumenau, por id decrescente: o 3 e o primeiro, o 1 e o segundo.
    assert_eq!(l[0].inteiro_ou("n", -1), 2, "id 1 em Blumenau");
    assert_eq!(l[1].inteiro_ou("n", -1), 1, "id 2, sozinho em Itajai");
    assert_eq!(l[2].inteiro_ou("n", -1), 1, "id 3 em Blumenau");

    // Funcao de janela que nao existe recusa NOMEANDO, em vez de numerar
    // qualquer coisa.
    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "janela":[{"funcao":"rank","apelido":"n"}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("rank"), "{e}");
    assert!(e.to_string().contains("row_number"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A PROJECAO VEM POR ULTIMO: `ordem` pode falar de coluna que a resposta
/// nao mostra -- `ORDER BY cidade` num `SELECT nome` e SQL legitimo.
#[test]
fn a_projecao_vem_depois_da_ordem() {
    let d = dir("projecao");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "ordem":[{"coluna":"cidade"},{"coluna":"id","desc":true}],
               "colunas":["nome"]"#,
    )
    .unwrap();
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    let nomes: Vec<String> = l
        .iter()
        .map(|x| x.texto_ou("nome", "").to_string())
        .collect();
    assert_eq!(nomes, vec!["caio", "ana", "bia", "duda"]);
    assert!(l[0].campo("cidade").is_none(), "a projecao nao cortou");
    assert!(l[0].campo("id").is_none(), "a projecao nao cortou");
    let _ = std::fs::remove_dir_all(&d);
}

/// `pular` e `max` recortam DEPOIS de ordenar, e `achadas` conta antes.
#[test]
fn o_recorte_vem_depois_da_ordem() {
    let d = dir("recorte");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "ordem":[{"coluna":"id","desc":true}],"pular":1,"max":2"#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![3, 2]);
    assert_eq!(
        r.inteiro_ou("achadas", -1),
        4,
        "achadas conta antes do corte"
    );
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2);
    let _ = std::fs::remove_dir_all(&d);
}

/// Um `consultar` sobre outro `consultar`: a composicao e composta.
#[test]
fn o_de_pode_ser_outro_consultar() {
    let d = dir("aninhado");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = consultar(
        &s,
        r#""de":{"op":"consultar","de":{"op":"varrer","tabela":"clientes"},
                     "expressao":"cidade = 'Blumenau'"},
               "ordem":[{"coluna":"id","desc":true}]"#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![3, 1]);
    let _ = std::fs::remove_dir_all(&d);
}

/// As recusas nomeiam: op que nao devolve linhas, coluna que nao existe,
/// aninhamento fundo demais.
#[test]
fn as_recusas_do_consultar_nomeiam() {
    let d = dir("recusa");
    let (s, _) = servidor(&d, Cadastro::default());

    let e = consultar(&s, r#""de":{"op":"inserir","tabela":"clientes"}"#).unwrap_err();
    assert!(e.to_string().contains("inserir"), "{e}");
    assert!(
        e.to_string().contains("varrer"),
        "a recusa nao lista o que da: {e}"
    );

    let e = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},"colunas":["lucro"]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("lucro"), "{e}");

    let e = consultar(&s, r#""ordem":[{"coluna":"id"}]"#).unwrap_err();
    assert!(e.to_string().contains("\"de\""), "{e}");

    // Aninhamento: nove niveis, um a mais que o teto.
    let mut fundo = r#"{"op":"varrer","tabela":"clientes"}"#.to_string();
    for _ in 0..9 {
        fundo = format!(r#"{{"op":"consultar","de":{fundo}}}"#);
    }
    let e = consultar(&s, &format!(r#""de":{fundo}"#)).unwrap_err();
    assert_eq!(e.nome(), "LIMITE_EXCEDIDO", "{e}");
    assert!(e.to_string().contains("aninhamento"), "{e}");

    // E o teto NAO fica preso: um pedido raso logo depois passa. Sem o
    // `Drop` do guarda, a thread ficaria contaminada e a proxima consulta
    // recusaria sem motivo.
    let ok = consultar(&s, r#""de":{"op":"varrer","tabela":"clientes"}"#).unwrap();
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 4);

    let _ = std::fs::remove_dir_all(&d);
}

/// **O direito por COLUNA tambem atravessa: a peneira e paga no
/// sub-pedido, e nao aqui.**
///
/// Quem nao le `salario` recebe as linhas sem ele -- porque o `varrer` de
/// dentro passou pelo `executar_derivado`, que peneira. E um `de` que e um
/// `agrupar` sobre a tabela restrita RECUSA, porque o `agrupar` e da
/// classe que recusa: o agregado fala da coluna sem ela aparecer.
#[test]
fn o_consultar_herda_o_direito_por_coluna() {
    let d = dir("coluna");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{"ler":true,
                   "colunas":{"cidade":{"ler":false}}}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&d, cadastro);

    let r = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"folha"}"#,
    )
    .expect("a tabela permitida tinha de passar");
    let l = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(l.len(), 1);
    assert!(
        l[0].campo("cidade").is_none(),
        "a coluna negada vazou: {l:?}"
    );
    assert_eq!(
        l[0].texto_ou("nome", ""),
        "segredo",
        "a peneira levou demais"
    );

    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b",
               "de":{"op":"agrupar","tabela":"folha","por":["cidade"]}"#,
    )
    .expect_err("o agrupar sobre a tabela restrita passou");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");

    let _ = std::fs::remove_dir_all(&d);
}
