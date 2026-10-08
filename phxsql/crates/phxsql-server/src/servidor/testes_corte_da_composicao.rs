//! **O corte que o teto impoe tem de APARECER na composicao (pedido 419).**
//!
//! A premissa que estes testes medem, e que desmente o diagnostico do pedido:
//! nenhuma das cinco operacoes de `OPS_QUE_DEVOLVEM_LINHAS` consegue devolver
//! MAIS que `recursos.max_linhas`, porque as quatro funcoes que as atendem
//! recortam por `self.limite(p)`, que ja e `min(max pedido, teto)`. Entao o
//! `if lista.len() as u64 > teto` de `linhas_do_sub_pedido` nunca dispara --
//! e trocar o `>` por `>=`, como o pedido prescrevia, nao acusaria o empate:
//! RECUSARIA justamente o caso em que o teto cortou, que e o que se quer
//! contar.
use super::*;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("corte-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `clientes` com DUAS linhas e `pedidos` com CINCO, sob um teto de 2.
///
/// As duas contagens sao escolhidas para separar os dois casos que um teto
/// unico juntaria: `clientes` cabe INTEIRA no teto (e cabe com igualdade,
/// `len() == teto`, que e o empate do pedido) e `pedidos` nao cabe. Assim
/// um teste mede o corte e o outro mede o nao-corte no mesmo servidor.
fn servidor(d: &std::path::Path, max_linhas: u64) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        max_linhas,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    for tab in ["clientes", "pedidos"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{tab}","colunas":[
                        {{"nome":"id","tipo":"Int4","obrigatoria":true}},
                        {{"nome":"nome","tipo":"Str(20)"}}],
                     "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                  "primario":true}},
                                {{"nome":"porNome","colunas":["nome"]}}]}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let grava = |tab: &str, ate: i64| {
        for id in 1..=ate {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{tab}","linha":
                            {{"id":{id},"nome":"igual"}}}}"#
                )),
                &Sessao::default(),
            )
            .unwrap();
        }
    };
    grava("clientes", 2);
    grava("pedidos", 5);
    s
}

fn consultar(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.executar(
        "consultar",
        &pedido(&format!(r#"{{"database":"b",{corpo}}}"#)),
        &Sessao::default(),
    )
}

/// **A PREMISSA, medida: o sub-pedido para EM `teto`, nunca acima dele --
/// e a composicao tem de dizer que parou.**
#[test]
fn o_sub_pedido_que_parou_no_teto_diz_que_parou() {
    let d = dir("teto");
    let s = servidor(&d, 2);
    let r = consultar(&s, r#""de":{"op":"varrer","tabela":"pedidos"}"#).unwrap();
    // EXATAMENTE o teto: o numero que o `>` de `linhas_do_sub_pedido`
    // espera (3 ou mais) nao existe.
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{r:?}");
    assert_eq!(r.inteiro_ou("achadas", -1), 2, "{r:?}");
    assert!(
        r.booleano_ou("truncado", false),
        "cortou tres das cinco linhas e nao disse: {r:?}"
    );
}

/// **O COMPORTAMENTO VELHO: quem cabe no teto nao muda em nada -- nem
/// quando cabe com igualdade.** E o teste que reprova o `>=` que o pedido
/// 419 prescrevia.
#[test]
fn a_composicao_que_cabe_no_teto_nao_recusa_nem_acusa_corte() {
    let d = dir("cabe");
    let s = servidor(&d, 2);
    let r = consultar(&s, r#""de":{"op":"varrer","tabela":"clientes"}"#)
        .expect("passou a recusar quem cabe no teto");
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{r:?}");
    assert!(
        !r.booleano_ou("truncado", true),
        "disse que cortou sem ter cortado: {r:?}"
    );
}

/// **O `max` que o sub-pedido PEDIU e um LIMIT, e nao um truncamento.**
/// `{"max":1,"ordem":[...]}` e o idioma documentado do `escalar`: bandeira
/// que acende em toda consulta correta e bandeira que ninguem le.
#[test]
fn o_max_pedido_pelo_sub_pedido_nao_conta_como_corte() {
    let d = dir("limit");
    let s = servidor(&d, 1000);
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "escalar":[{"nome":"topo","campo":"id",
                           "de":{"op":"varrer","tabela":"pedidos","max":1,
                                 "ordem":[{"coluna":"id"}]}}]"#,
    )
    .unwrap();
    assert!(
        !r.booleano_ou("truncado", true),
        "o LIMIT de quem pediu virou truncamento: {r:?}"
    );
}

/// **O lado de DENTRO do `em` cortado pelo teto aparece.** Um `IN` sobre
/// meio conjunto responde «nao casa» a linha que casava -- numero errado
/// com cara de resposta, que e o estrago que o 419 nomeia.
#[test]
fn o_em_sobre_conjunto_cortado_diz_que_cortou() {
    let d = dir("em");
    let s = servidor(&d, 2);
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "em":[{"coluna":"id","de":{"op":"varrer","tabela":"pedidos"},
                      "campo":"id"}]"#,
    )
    .unwrap();
    assert!(
        r.booleano_ou("truncado", false),
        "o IN leu duas das cinco chaves e nao disse: {r:?}"
    );
}

/// O mesmo pelo `existe` -- a semijunção sobre um lado cortado.
#[test]
fn o_existe_sobre_lado_cortado_diz_que_cortou() {
    let d = dir("existe");
    let s = servidor(&d, 2);
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "existe":[{"apelido":"p","de":{"op":"varrer","tabela":"pedidos"},
                          "em":[{"esquerda":"id","direita":"p.id"}]}]"#,
    )
    .unwrap();
    assert!(
        r.booleano_ou("truncado", false),
        "a semijunção viu dois dos cinco e nao disse: {r:?}"
    );
}

/// O `buscar` avisa por outro dialeto -- `encontrados` acima das linhas
/// que vieram --, e a composicao tem de entender esse tambem.
#[test]
fn o_buscar_cortado_no_indice_diz_que_cortou() {
    let d = dir("buscar");
    let s = servidor(&d, 2);
    let r = consultar(
        &s,
        r#""de":{"op":"buscar","tabela":"pedidos","indice":"porNome",
                     "chave":["igual"]}"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2, "{r:?}");
    assert!(
        r.booleano_ou("truncado", false),
        "o indice achou cinco, a pagina levou duas, e ninguem disse: {r:?}"
    );
}

/// **O lado DIREITO de uma junção cortado pelo teto aparece.** O lado de
/// fora cabe inteiro; a junção casa contra meia tabela e produziria menos
/// linhas do que existem, sem uma palavra.
#[test]
fn o_lado_direito_da_juncao_cortado_diz_que_cortou() {
    let d = dir("juntar");
    let s = servidor(&d, 2);
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},"apelido":"c",
               "juntar":[{"apelido":"p","tipo":"interno",
                          "de":{"op":"varrer","tabela":"pedidos"},
                          "em":[{"esquerda":"c.id","direita":"p.id"}]}]"#,
    )
    .unwrap();
    assert!(
        r.booleano_ou("truncado", false),
        "casou contra duas das cinco linhas e nao disse: {r:?}"
    );
}

/// **O `max` de FORA nao apaga o corte de dentro.** O `escalar` pede
/// `max: 1` -- um LIMIT legitimo --, e o sub-pedido dele e um `consultar`
/// que ja parou no teto. E o caso que separa o crivo (`parou_no_teto`
/// filtra o `max` pedido) da propagacao (o `truncado` de um `consultar`
/// viaja inteiro): sem a segunda, o LIMIT de cima esconderia o corte de
/// baixo, e o numero sairia errado com cara de resposta.
#[test]
fn o_limit_de_fora_nao_esconde_o_corte_de_dentro() {
    let d = dir("escalar");
    let s = servidor(&d, 2);
    let r = consultar(
        &s,
        r#""de":{"op":"varrer","tabela":"clientes"},
               "escalar":[{"nome":"topo","campo":"id",
                           "de":{"op":"consultar","max":1,
                                 "colunas":["id"],
                                 "de":{"op":"varrer","tabela":"pedidos"}}}]"#,
    )
    .unwrap();
    assert!(
        r.booleano_ou("truncado", false),
        "o max de fora apagou o corte de tres linhas la dentro: {r:?}"
    );
}

/// **O corte viaja pelos niveis.** Um `consultar` dentro de outro: o de
/// fora cabe no teto e mesmo assim tem de dizer que o de dentro nao coube.
#[test]
fn o_corte_de_tres_niveis_abaixo_sobe_ate_a_resposta() {
    let d = dir("niveis");
    let s = servidor(&d, 2);
    let r = consultar(
        &s,
        r#""de":{"op":"consultar","de":{"op":"consultar",
                     "de":{"op":"varrer","tabela":"pedidos"}}}"#,
    )
    .unwrap();
    assert!(
        r.booleano_ou("truncado", false),
        "o corte morreu no caminho: {r:?}"
    );
}

/// O braço de `unir` que chegou como PEDIDO passa pelo mesmo
/// `linhas_do_sub_pedido`, e e o SEXTO chamador dele: o corte do braço
/// tem de entrar no `truncado` que a uniao ja publica.
#[test]
fn o_braco_da_uniao_cortado_entra_no_truncado_dela() {
    let d = dir("uniao");
    let s = servidor(&d, 2);
    let r = s
        .executar(
            "unir",
            &pedido(
                r#"{"database":"b","partes":[
                         {"op":"varrer","tabela":"clientes","colunas":["nome"]},
                         {"op":"varrer","tabela":"pedidos","colunas":["nome"]}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert!(
        r.booleano_ou("truncado", false),
        "o braço leu duas das cinco e a uniao disse que estava inteira: {r:?}"
    );
}

/// E o comportamento velho da uniao: dois braços que cabem nao acusam
/// corte nenhum.
#[test]
fn a_uniao_de_bracos_que_cabem_nao_acusa_corte() {
    let d = dir("uniao-cabe");
    let s = servidor(&d, 1000);
    let r = s
        .executar(
            "unir",
            &pedido(
                r#"{"database":"b","partes":[
                         {"op":"varrer","tabela":"clientes","colunas":["nome"]},
                         {"op":"varrer","tabela":"pedidos","colunas":["nome"]}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    // `distinta` e o padrao: os ids 1 e 2 repetem entre as duas tabelas.
    assert_eq!(r.inteiro_ou("quantas", -1), 5, "{r:?}");
    assert_eq!(r.inteiro_ou("repetidas", -1), 2, "{r:?}");
    assert!(
        !r.booleano_ou("truncado", true),
        "disse que cortou sem cortar: {r:?}"
    );
}
