//! A op `agrupar` -- o `GROUP BY` pelo protocolo.
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("agrupar-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Uma base `loja` com `vendas` (o que se agrupa) e `folha` (a negada).
fn servidor(d: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        cadastro,
        max_linhas: 10_000,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let ses = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &ses)
        .unwrap();
    for tab in ["vendas", "folha"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"loja","tabela":"{tab}","colunas":[
                        {{"nome":"id","tipo":"Int8","obrigatoria":true}},
                        {{"nome":"cidade","tipo":"Str(30)"}},
                        {{"nome":"total","tipo":"Decimal(12,2)"}}],
                     "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                  "primario":true}}]}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    // 10,00 + 0,01 em Blumenau; 20,50 em Itajai; uma linha sem cidade.
    for (id, cidade, total) in [
        (1, r#""Blumenau""#, r#""10.00""#),
        (2, r#""Itajai""#, r#""20.50""#),
        (3, r#""Blumenau""#, r#""0.01""#),
        (4, "null", r#""5.00""#),
        (5, r#""Blumenau""#, "null"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"loja","tabela":"vendas",
                         "linha":{{"id":{id},"cidade":{cidade},"total":{total}}}}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"loja","tabela":"folha","linha":{"id":1,"cidade":"x","total":"9.99"}}"#,
        ),
        &ses,
    )
    .unwrap();
    s
}

fn agrupar(s: &Arc<Servidor>, extra: &str) -> Result<Json> {
    s.executar(
        "agrupar",
        &pedido(&format!(
            r#"{{"database":"loja","tabela":"vendas"{extra}}}"#
        )),
        &Sessao::default(),
    )
}

fn linhas(r: &Json) -> Vec<Json> {
    r.campo("linhas").and_then(Json::lista).unwrap().to_vec()
}

/// **A PROVA REAL: a soma sai EXATA, e o teste mede o valor -- nao se
/// agrupou.**
///
/// 10,00 + 0,01 e 10,01. Uma soma em `f64` daria `10.009999999999999`, e
/// um teste que so contasse os grupos passaria com esse defeito reposto.
/// A linha de `total` nulo conta como LINHA (o `COUNT(*)`) e nao entra na
/// SOMA -- somar «sem valor» como zero afundaria a media.
#[test]
fn a_soma_por_cidade_e_exata_e_o_nulo_conta_linha_sem_somar() {
    let d = dir("soma");
    let s = servidor(&d, Cadastro::default());
    let r = agrupar(
        &s,
        r#","por":["cidade"],
               "agregados":[{"funcao":"contagem","apelido":"n"},
                            {"funcao":"soma","coluna":"total","apelido":"total"},
                            {"funcao":"media","coluna":"total","apelido":"media"}],
               "ordem":[{"coluna":"cidade"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("grupos", -1), 3, "Blumenau, Itajai e o nulo");
    assert_eq!(r.inteiro_ou("examinadas", -1), 5);
    let l = linhas(&r);
    // A ordem por `cidade` poe o NULO primeiro (ele compara menor).
    assert_eq!(l[1].texto_ou("cidade", ""), "Blumenau");
    assert_eq!(l[1].inteiro_ou("n", -1), 3, "a linha de total nulo e linha");
    assert_eq!(
        l[1].texto_ou("total", ""),
        "10.01",
        "a soma de Decimal perdeu centavo"
    );
    // A media divide pelas linhas que TEM valor (duas), e nao pelas tres.
    assert_eq!(l[1].texto_ou("media", ""), "5.00");
    assert_eq!(l[2].texto_ou("cidade", ""), "Itajai");
    assert_eq!(l[2].texto_ou("total", ""), "20.50");
    let _ = std::fs::remove_dir_all(&d);
}

/// `por` vazio e UM grupo: e o `SELECT COUNT(*), SUM(total) FROM vendas`.
#[test]
fn sem_por_e_um_grupo_so() {
    let d = dir("um");
    let s = servidor(&d, Cadastro::default());
    let r = agrupar(
        &s,
        r#","agregados":[{"funcao":"contagem","apelido":"n"},
                             {"funcao":"soma","coluna":"total","apelido":"total"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("grupos", -1), 1);
    let l = linhas(&r);
    assert_eq!(l[0].inteiro_ou("n", -1), 5);
    assert_eq!(l[0].texto_ou("total", ""), "35.51");
    let _ = std::fs::remove_dir_all(&d);
}

/// O `tendo` peneira o RESULTADO, e nao a linha crua.
///
/// Com `n > 1` so Blumenau sobra. E `grupos` passa a contar o que sobrou:
/// e o tamanho do resultado, que e o que quem pagina precisa saber.
#[test]
fn o_tendo_peneira_o_grupo_e_nao_a_linha() {
    let d = dir("tendo");
    let s = servidor(&d, Cadastro::default());
    let r = agrupar(
        &s,
        r#","por":["cidade"],
               "agregados":[{"funcao":"contagem","apelido":"n"}],
               "tendo":"n > 1""#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("grupos", -1), 1);
    let l = linhas(&r);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].texto_ou("cidade", ""), "Blumenau");
    assert_eq!(l[0].inteiro_ou("n", -1), 3);
    let _ = std::fs::remove_dir_all(&d);
}

/// O `onde`/`expressao` peneira a linha CRUA, ANTES de agrupar.
///
/// A ordem dos dois nao e detalhe: com `total > 5` avaliado depois do
/// grupo, Blumenau sairia com `n = 3`; avaliado antes, sai com `n = 1`.
#[test]
fn a_expressao_peneira_antes_de_agrupar() {
    let d = dir("antes");
    let s = servidor(&d, Cadastro::default());
    let r = agrupar(
        &s,
        r#","por":["cidade"],
               "agregados":[{"funcao":"contagem","apelido":"n"}],
               "expressao":"total > 5","ordem":[{"coluna":"cidade"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("examinadas", -1), 5, "a varredura olha todas");
    assert_eq!(r.inteiro_ou("consideradas", -1), 2, "so duas passam");
    let l = linhas(&r);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].texto_ou("cidade", ""), "Blumenau");
    assert_eq!(l[0].inteiro_ou("n", -1), 1);
    let _ = std::fs::remove_dir_all(&d);
}

/// A `ordem` fala dos nomes da linha AGREGADA -- inclusive dos apelidos.
#[test]
fn a_ordem_enxerga_o_apelido() {
    let d = dir("ordem");
    let s = servidor(&d, Cadastro::default());
    let r = agrupar(
        &s,
        r#","por":["cidade"],
               "agregados":[{"funcao":"contagem","apelido":"n"}],
               "ordem":[{"coluna":"n","desc":true}]"#,
    )
    .unwrap();
    let l = linhas(&r);
    assert_eq!(l[0].texto_ou("cidade", ""), "Blumenau");
    assert_eq!(l[0].inteiro_ou("n", -1), 3);
    let _ = std::fs::remove_dir_all(&d);
}

/// Cada recusa diz O QUE esta errado, com o nome dentro.
#[test]
fn as_recusas_nomeiam() {
    let d = dir("recusa");
    let s = servidor(&d, Cadastro::default());

    // Coluna que nao existe no agregado.
    let e = agrupar(&s, r#","agregados":[{"funcao":"soma","coluna":"lucro"}]"#).unwrap_err();
    assert!(e.to_string().contains("lucro"), "{e}");

    // Soma sem coluna: so a contagem dispensa valor.
    let e = agrupar(&s, r#","agregados":[{"funcao":"soma"}]"#).unwrap_err();
    assert!(e.to_string().contains("coluna"), "{e}");

    // `tendo` falando de coluna crua: ela nao existe depois de agrupar.
    let e = agrupar(
        &s,
        r#","por":["cidade"],"agregados":[{"funcao":"contagem","apelido":"n"}],
               "tendo":"total > 5""#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("total"), "{e}");
    assert!(
        e.to_string().contains('n'),
        "a recusa tem de listar o que da: {e}"
    );

    // Apelido repetido: um dos dois ficaria invisivel na resposta.
    let e = agrupar(
        &s,
        r#","agregados":[{"funcao":"contagem","apelido":"n"},
                             {"funcao":"soma","coluna":"total","apelido":"n"}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("\"n\""), "{e}");

    // Apelido colidindo com a coluna de agrupamento.
    let e = agrupar(
        &s,
        r#","por":["cidade"],"agregados":[{"funcao":"contagem","apelido":"cidade"}]"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("cidade"), "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// O teto de GRUPOS recusa nomeando, em vez de engasgar a maquina.
#[test]
fn o_teto_de_grupos_recusa_nomeando() {
    let d = dir("teto");
    let s = servidor(&d, Cadastro::default());
    // Um servidor com `max_linhas` de 2 e tres cidades distintas.
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        max_linhas: 2,
        ..Config::default()
    };
    let s2 = Servidor::novo(c).unwrap();
    let e = s2
        .executar(
            "agrupar",
            &pedido(r#"{"database":"loja","tabela":"vendas","por":["id"]}"#),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("max_linhas"), "{e}");
    assert_eq!(e.nome(), "LIMITE_EXCEDIDO", "{e}");
    drop(s);
    let _ = std::fs::remove_dir_all(&d);
}

/// **O portao continua sendo UM: `agrupar` nomeia tabela no campo que ele
/// ja le, e a tabela negada para no portao geral.**
///
/// Pelo `despachar`, que e por onde o pedido entra de verdade.
#[test]
fn o_agrupar_da_tabela_negada_para_no_portao() {
    let d = dir("portao");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap();
    let s = servidor(&d, cadastro.clone());
    let mut ses = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let corpo = |tab: &str| {
        format!(
            r#"{{"token":"t","op":"agrupar","database":"loja","tabela":"{tab}",
                     "por":["cidade"]}}"#
        )
    };
    let (_, _, ok) = s.despachar(&corpo("vendas"), &mut ses, "127.0.0.1");
    ok.expect("a tabela permitida tinha de passar");
    let (_, _, negado) = s.despachar(&corpo("folha"), &mut ses, "127.0.0.1");
    let e = negado.expect_err("agrupou a tabela negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("folha"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
