//! A op `diferencas` -- o TERCEIRO IRMAO da conferencia propria.
use super::*;
use crate::usuarios::Cadastro;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("dif-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `hoje` e `ontem` com a MESMA forma, e `folha` com outra (para provar a
/// recusa por coluna diferente).
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
    for tab in ["hoje", "ontem", "folha"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{tab}","colunas":[
                        {{"nome":"id","tipo":"Int4","obrigatoria":true}},
                        {{"nome":"nome","tipo":"Str(20)"}}],
                     "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                  "primario":true}}]}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    // Uma tabela com outra forma, para a recusa por coluna diferente.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"outra_forma","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"},
                    {"nome":"extra","tipo":"Int4"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    // Uma com indice REPETIVEL, para a recusa do indice nao unico.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"sem_chave","colunas":[
                    {"nome":"id","tipo":"Int4","obrigatoria":true},
                    {"nome":"nome","tipo":"Str(20)"}],
                 "indices":[{"nome":"porNome","colunas":["nome"]}]}"#,
        ),
        &ses,
    )
    .unwrap();
    let grava = |tab: &str, linhas: &[(i64, &str)]| {
        for (id, nome) in linhas {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{tab}","linha":{{"id":{id},"nome":"{nome}"}}}}"#
                )),
                &ses,
            )
            .unwrap();
        }
    };
    grava("hoje", &[(1, "ana"), (2, "bia"), (3, "caio")]);
    grava("ontem", &[(1, "ana"), (2, "BIA"), (4, "duda")]);
    grava("folha", &[(1, "segredo")]);
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

fn dif(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.executar(
        "diferencas",
        &pedido(&format!(r#"{{"database":"b",{corpo}}}"#)),
        &Sessao::default(),
    )
}

fn ids(r: &Json, campo: &str) -> Vec<i64> {
    r.campo(campo)
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|c| {
            c.lista()
                .and_then(|l| l.first().and_then(Json::inteiro))
                .unwrap_or(-1)
        })
        .collect()
}

/// **A PROVA REAL: as tres listas, e a coluna que mudou.**
///
/// O 3 so esta em `hoje`, o 4 so em `ontem`, o 2 esta nos dois e difere no
/// `nome`, e o 1 e igual. Um teste que so contasse as listas passaria com
/// os dois lados TROCADOS -- por isso ele confere QUEM esta em cada uma.
#[test]
fn as_tres_listas_dizem_de_que_lado_esta_cada_um() {
    let d = dir("tres");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = dif(&s, r#""a":"hoje","b":"ontem""#).expect("nao rodou");
    assert_eq!(r.inteiro_ou("iguais", -1), 1);
    assert_eq!(ids(&r, "so_em_a"), vec![3], "o 3 so existe em hoje");
    assert_eq!(ids(&r, "so_em_b"), vec![4], "o 4 so existe em ontem");
    assert!(!r.booleano_ou("truncado", true));

    let difs = r.campo("diferentes").and_then(Json::lista).unwrap();
    assert_eq!(difs.len(), 1);
    let colunas: Vec<String> = difs[0]
        .campo("colunas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(|x| x.texto().map(str::to_string))
        .collect();
    assert_eq!(colunas, vec!["nome".to_string()], "nao disse qual coluna");
    assert_eq!(difs[0].campo("a").unwrap().texto_ou("nome", ""), "bia");
    assert_eq!(difs[0].campo("b").unwrap().texto_ou("nome", ""), "BIA");

    // E trocando os lados, as duas listas trocam -- o controle que impede
    // o teste de passar com os lados invertidos.
    let r = dif(&s, r#""a":"ontem","b":"hoje""#).unwrap();
    assert_eq!(ids(&r, "so_em_a"), vec![4]);
    assert_eq!(ids(&r, "so_em_b"), vec![3]);
    let _ = std::fs::remove_dir_all(&d);
}

/// `max` corta CADA lista, e a resposta diz `truncado`.
#[test]
fn o_max_corta_e_a_resposta_diz() {
    let d = dir("max");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = dif(&s, r#""a":"hoje","b":"folha","max":1"#);
    // `folha` tem a mesma forma de `hoje`, entao a comparacao roda.
    let r = r.expect("nao rodou");
    assert!(r.booleano_ou("truncado", false) || r.inteiro_ou("iguais", -1) >= 0);
    // Com max 1 e duas linhas so em `hoje` (2 e 3), a lista corta.
    assert_eq!(r.campo("so_em_a").and_then(Json::lista).unwrap().len(), 1);
    assert!(r.booleano_ou("truncado", false), "cortou e nao disse");
    let _ = std::fs::remove_dir_all(&d);
}

/// Duas iguais nao tem diferenca -- o controle da bateria.
#[test]
fn duas_iguais_nao_tem_diferenca() {
    let d = dir("iguais");
    let (s, _) = servidor(&d, Cadastro::default());
    let r = dif(&s, r#""a":"hoje","b":"hoje""#).unwrap();
    assert_eq!(r.inteiro_ou("iguais", -1), 3);
    assert!(r
        .campo("diferentes")
        .and_then(Json::lista)
        .unwrap()
        .is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

/// As recusas nomeiam: coluna diferente, indice que nao existe, indice
/// repetivel.
#[test]
fn as_recusas_nomeiam() {
    let d = dir("recusa");
    let (s, _) = servidor(&d, Cadastro::default());

    let e = dif(&s, r#""a":"hoje","b":"outra_forma""#).unwrap_err();
    let t = e.to_string();
    assert!(t.contains("extra"), "a recusa nao diz a coluna: {t}");
    assert!(t.contains("iguais"), "a recusa nao diz por que: {t}");

    let e = dif(&s, r#""a":"hoje","b":"ontem","indice":"nao_existe""#).unwrap_err();
    assert!(e.to_string().contains("nao_existe"), "{e}");

    let e = dif(&s, r#""a":"sem_chave","b":"sem_chave","indice":"porNome""#).unwrap_err();
    assert!(e.to_string().contains("unico"), "{e}");

    let e = dif(&s, r#""a":"hoje""#).unwrap_err();
    assert!(e.to_string().contains("\"b\""), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **A CONFERENCIA PROPRIA: `diferencas` nao tem campo `"tabela"`, entao o
/// portao geral nao ve as tabelas dela.**
///
/// Este e o teste que viaja com a operacao -- e o unico que acusa se
/// alguem limpar a conferencia por ela parecer duplicacao do portao geral.
///
/// PROVA REAL: tirando o `for alvo in [&na, &nb]` do `op_diferencas`, a
/// folha entra pelos dois lados e este teste REPROVA nas duas -- enquanto
/// o controle, as tabelas permitidas, continua respondendo.
#[test]
fn diferencas_nao_e_a_porta_dos_fundos() {
    let d = dir("porta");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"folha":{}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&d, cadastro);
    let pede = |a: &str, b: &str| -> Result<Json> {
        let mut sessao = Sessao {
            usuario: ses.usuario.clone(),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t","op":"diferencas","database":"b","a":"{a}","b":"{b}"}}"#),
            &mut sessao,
            "127.0.0.1",
        );
        r
    };
    pede("hoje", "ontem").expect("as tabelas permitidas foram barradas");
    for (a, b) in [("folha", "hoje"), ("hoje", "folha")] {
        let e = pede(a, b).expect_err("leu a tabela negada");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{a}/{b}: {e}");
        assert!(format!("{e}").contains("folha"), "{a}/{b}: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// E o DIREITO POR COLUNA recusa a tabela restrita, pelo mesmo argumento
/// do `juntar`: a resposta traz a linha inteira dos dois lados, e a lista
/// `colunas` responde sobre a coluna negada mesmo sem mostra-la.
#[test]
fn diferencas_recusa_a_tabela_com_regra_de_coluna() {
    let d = dir("coluna");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"tabelas":{"ontem":{"ler":true,
                   "colunas":{"nome":{"ler":false}}}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&d, cadastro);
    let mut sessao = Sessao {
        usuario: ses.usuario.clone(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"diferencas","database":"b","a":"hoje","b":"ontem"}"#,
        &mut sessao,
        "127.0.0.1",
    );
    let e = r.expect_err("comparou contra a tabela restrita");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    // O controle: duas tabelas SEM regra de coluna continuam comparando.
    let (_, _, ok) = s.despachar(
        r#"{"token":"t","op":"diferencas","database":"b","a":"hoje","b":"folha"}"#,
        &mut sessao,
        "127.0.0.1",
    );
    ok.expect("as tabelas sem regra de coluna foram barradas");
    let _ = std::fs::remove_dir_all(&d);
}
