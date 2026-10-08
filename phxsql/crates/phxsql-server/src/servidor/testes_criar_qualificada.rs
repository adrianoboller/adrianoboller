use super::*;

fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `filial.clientes` e o schema `filial` mais a tabela `clientes`.
///
/// Antes desta correcao a criacao tomava o ponto como parte do NOME e
/// gravava `filial.clientes.reg` na raiz do banco. O servidor respondia
/// "criada", e nenhuma outra operacao conseguia abrir a tabela -- toda
/// leitura separa o ponto, e so a criacao nao separava. Uma tabela que
/// nasce inalcancavel e pior do que um erro.
#[test]
fn criar_com_nome_qualificado_cai_no_schema() {
    let dir = DirTemp::novo("qualif");
    let s = servidor(&dir);
    let sessao = Sessao::default();

    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
        .unwrap();
    let r = s
        .executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"loja","tabela":"filial.clientes",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.texto_ou("schema", ""), "filial");
    assert_eq!(r.texto_ou("tabela", ""), "filial.clientes");

    // Os arquivos foram para o diretorio do schema, com o nome curto.
    assert!(
        dir.join("loja/filial/clientes.reg").exists(),
        "o .reg nao caiu no schema"
    );
    assert!(
        !dir.join("loja/filial.clientes.reg").exists(),
        "o ponto virou parte do nome do arquivo de novo"
    );

    // E a prova que importa: o que foi criado da para abrir e gravar.
    s.executar(
        "inserir",
        &pedido(r#"{"database":"loja","tabela":"filial.clientes","linha":{"id":1}}"#),
        &sessao,
    )
    .unwrap();
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"loja","tabela":"filial.clientes"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(v.campo("linhas").and_then(Json::lista).unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

/// PROVA REAL da infraestrutura dos tres tipos de database. Nos dois
/// sentidos: a hive nasce valida e reserva o tipo, mas operar tabela nela
/// RECUSA ("motor em construcao") -- sem o portao, a hive criaria um `.reg`
/// relacional calada. O padrao, ao lado, opera como sempre.
#[test]
fn criar_database_hive_reserva_o_tipo_mas_recusa_tabela() {
    let dir = DirTemp::novo("tres-tipos");
    let s = servidor(&dir);
    let ses = Sessao::default();

    // A hive nasce valida: tipo no marcador, motor_pronto=false e a nota
    // que avisa antes de o cliente tentar a primeira tabela.
    let r = s
        .executar(
            "criar_database",
            &pedido(r#"{"database":"cfg","tipo":"hive"}"#),
            &ses,
        )
        .unwrap();
    assert_eq!(r.texto_ou("tipo", ""), "hive");
    assert!(!r.booleano_ou("motor_pronto", true));
    assert!(r.texto_ou("nota", "").contains("construcao"));

    // Operar tabela numa hive recusa, e diz por que. Defeito reposto (sem
    // o portao): esta criacao responderia "criada".
    let erro = s
        .executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"cfg","tabela":"t",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &ses,
        )
        .unwrap_err();
    assert!(
        erro.to_string().contains("construcao"),
        "esperava recusa de motor em construcao, veio {erro}"
    );

    // Tipo desconhecido e' ERRO na criacao, nao palpite.
    assert!(s
        .executar(
            "criar_database",
            &pedido(r#"{"database":"g","tipo":"grafo"}"#),
            &ses,
        )
        .is_err());

    // Padrao (sem `tipo`) continua criando e operando tabela como sempre.
    let p = s
        .executar("criar_database", &pedido(r#"{"database":"rel"}"#), &ses)
        .unwrap();
    assert_eq!(p.texto_ou("tipo", ""), "padrao");
    assert!(p.booleano_ou("motor_pronto", false));
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"rel","tabela":"t",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
        ),
        &ses,
    )
    .unwrap();

    std::fs::remove_dir_all(&dir).unwrap();
}

/// O campo `schema` continua valendo, e vale igual.
#[test]
fn o_campo_schema_continua_valendo() {
    let dir = DirTemp::novo("qualif2");
    let s = servidor(&dir);
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","schema":"matriz","tabela":"estoque",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    assert!(dir.join("loja/matriz/estoque.reg").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Dizer duas coisas diferentes e erro, e nao "uma delas ganha".
#[test]
fn nome_e_campo_em_desacordo_param_a_criacao() {
    let dir = DirTemp::novo("qualif3");
    let s = servidor(&dir);
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
        .unwrap();
    let e = s
        .executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"loja","schema":"matriz","tabela":"filial.estoque",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &sessao,
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("escolha um dos dois"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}
