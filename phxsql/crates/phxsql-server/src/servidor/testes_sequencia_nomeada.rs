//! Direito no nivel da TABELA.
//!
//! Ate a 0.17.0 a permissao parava na base: quem lia a base lia todas as
//! tabelas dela. A folha de pagamento e a tabela de clientes moram no mesmo
//! banco porque o negocio e um so, e o direito de ler as duas nao e o mesmo.
//!
//! O que estes testes travam, em ordem de importancia:
//!
//! 1. a regra da tabela **tira** de quem le a base inteira;
//! 2. a regra da tabela **da** a quem nao le a base nenhuma;
//! 3. `juntar` e `unir` **nao sao a porta dos fundos** -- as tabelas delas nao
//!    passam pelo campo que o portao geral olha;
/* =========================================================== pedido 229
A SEQUENCIA NOMEADA pelo protocolo.

O `.seq` vive em `phxsql_store::sequencia`, e os testes de formato moram la.
Aqui se prova o que so o servidor decide: as quatro operacoes, o braco
nomeado do `ajustar_sequencia`, o espaco de nomes dividido com a tabela, o
portao de permissao por atividade, e o numero grande saindo como texto. */
use super::*;
use crate::usuarios::Cadastro;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn cadastro(bases: &str) -> Cadastro {
    Cadastro::de_json(&pedido(&format!(
        r#"{{"usuarios":[{{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
    )))
    .unwrap()
}

/// Um servidor com a base `b` e a tabela `clientes`.
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
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

/// Pelo `despachar`, que e onde mora o portao de permissao.
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

fn dono(s: &Arc<Servidor>, op: &str, corpo: &str) -> Result<Json> {
    s.executar(op, &pedido(corpo), &Sessao::default())
}

/// O ciclo inteiro: nasce, numera, mostra, ajusta, lista, morre.
#[test]
fn nasce_numera_ajusta_lista_e_morre() {
    let dir = DirTemp::novo("seqn-ciclo");
    let (s, _) = servidor(&dir, cadastro("{}"));
    let r = dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"nf","inicio":"1000","passo":10}"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("proximo", -1), 1000);
    assert_eq!(r.inteiro_ou("passo", -1), 10);
    for esperado in [1000, 1010, 1020] {
        let r = dono(
            &s,
            "proximo_da_sequencia",
            r#"{"database":"b","sequencia":"nf"}"#,
        )
        .unwrap();
        assert_eq!(r.inteiro_ou("valor", -1), esperado);
    }
    let r = dono(&s, "sequencia", r#"{"database":"b","sequencia":"nf"}"#).unwrap();
    assert_eq!(r.inteiro_ou("proximo", -1), 1030);
    assert_eq!(r.inteiro_ou("entregues", -1), 3);

    // O ajuste entra pela MESMA porta da coluna Sequence, com o campo
    // `sequencia` no lugar de `tabela`.
    let r = dono(
        &s,
        "ajustar_sequencia",
        r#"{"database":"b","sequencia":"nf","proxima":5}"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("antes", -1), 1030);
    assert!(
        !r.texto_ou("aviso", "").is_empty(),
        "andou para tras: avisa"
    );
    assert_eq!(
        dono(
            &s,
            "proximo_da_sequencia",
            r#"{"database":"b","sequencia":"nf"}"#
        )
        .unwrap()
        .inteiro_ou("valor", -1),
        5
    );

    // Aparece no `sequencias` do banco, em lista propria.
    let r = dono(&s, "sequencias", r#"{"database":"b"}"#).unwrap();
    let nomeadas = r.campo("nomeadas").and_then(Json::lista).unwrap();
    assert_eq!(nomeadas.len(), 1);
    assert_eq!(nomeadas[0].texto_ou("sequencia", ""), "nf");
    assert_eq!(
        r.inteiro_ou("total", -1),
        1,
        "o total continua sendo o das tabelas"
    );

    // Excluir pede o nome repetido, e depois nao ha mais numero.
    assert!(dono(
        &s,
        "excluir_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#
    )
    .is_err());
    dono(
        &s,
        "excluir_sequencia",
        r#"{"database":"b","sequencia":"nf","confirmar":"nf"}"#,
    )
    .unwrap();
    let e = dono(
        &s,
        "proximo_da_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#,
    )
    .unwrap_err();
    assert!(matches!(e, PhxError::NaoEncontrado(_)), "{e}");
    assert!(!dir.0.join("b").join("nf.seq").exists());
}

/// `filial.nf` mora na pasta do schema, como a tabela -- e o schema tem
/// de existir antes (PostgreSQL e MariaDB convergem).
#[test]
fn o_nome_qualificado_vai_para_o_schema() {
    let dir = DirTemp::novo("seqn-schema");
    let (s, _) = servidor(&dir, cadastro("{}"));
    let e = dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"filial.nf"}"#,
    )
    .unwrap_err();
    assert!(matches!(e, PhxError::NaoEncontrado(_)), "{e}");
    dono(&s, "criar_schema", r#"{"database":"b","schema":"filial"}"#).unwrap();
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"filial.nf"}"#,
    )
    .unwrap();
    assert!(dir.0.join("b").join("filial").join("nf.seq").is_file());
    assert_eq!(
        dono(
            &s,
            "proximo_da_sequencia",
            r#"{"database":"b","sequencia":"filial.nf"}"#
        )
        .unwrap()
        .inteiro_ou("valor", -1),
        1
    );
    let r = dono(&s, "sequencias", r#"{"database":"b"}"#).unwrap();
    let nomeadas = r.campo("nomeadas").and_then(Json::lista).unwrap();
    assert_eq!(nomeadas[0].texto_ou("sequencia", ""), "filial.nf");
}

/// **Tabela e sequencia dividem o espaco de nomes**, nos dois sentidos --
/// como no PostgreSQL e no MariaDB, onde a sequencia E uma relacao.
///
/// Reponha o defeito tirando o `crate::sequencia::existe` do
/// `criar_tabela_adiando_o_fsync` (catalogo.rs): a tabela `nf` nasce ao
/// lado da sequencia `nf`, e a primeira assercao cai.
#[test]
fn tabela_e_sequencia_nao_dividem_o_nome() {
    let dir = DirTemp::novo("seqn-nomes");
    let (s, _) = servidor(&dir, cadastro("{}"));
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#,
    )
    .unwrap();
    let e = dono(
        &s,
        "criar_tabela",
        r#"{"database":"b","tabela":"nf",
                "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
    )
    .unwrap_err();
    assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
    assert!(e.to_string().contains("sequencia"), "{e}");
    let e = dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"clientes"}"#,
    )
    .unwrap_err();
    assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
    assert!(e.to_string().contains("tabela"), "{e}");
}

/// O portao por atividade: quem so insere numera e nao cria; quem so le
/// ve o estado e nao numera; apagar exige administrar.
#[test]
fn o_portao_de_cada_operacao() {
    let dir = DirTemp::novo("seqn-portao");
    let (s, insere) = servidor(&dir, cadastro(r#"{"b":{"inserir":true}}"#));
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#,
    )
    .unwrap();
    assert!(pede(
        &s,
        &insere,
        r#""op":"criar_sequencia","database":"b","sequencia":"x""#
    )
    .is_err());
    assert_eq!(
        pede(
            &s,
            &insere,
            r#""op":"proximo_da_sequencia","database":"b","sequencia":"nf""#
        )
        .unwrap()
        .inteiro_ou("valor", -1),
        1
    );
    assert!(pede(
        &s,
        &insere,
        r#""op":"sequencia","database":"b","sequencia":"nf""#
    )
    .is_err());
    assert!(pede(
        &s,
        &insere,
        r#""op":"excluir_sequencia","database":"b","sequencia":"nf","confirmar":"nf""#
    )
    .is_err());

    let dir = DirTemp::novo("seqn-portao-le");
    let (s, le) = servidor(&dir, cadastro(r#"{"b":{"ler":true}}"#));
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#,
    )
    .unwrap();
    assert_eq!(
        pede(
            &s,
            &le,
            r#""op":"sequencia","database":"b","sequencia":"nf""#
        )
        .unwrap()
        .inteiro_ou("proximo", -1),
        1
    );
    assert!(pede(
        &s,
        &le,
        r#""op":"proximo_da_sequencia","database":"b","sequencia":"nf""#
    )
    .is_err());
}

/// O numero acima de 2^53 sai como TEXTO, pela mesma regra do `Int8`, e
/// entra como texto no `inicio`.
#[test]
fn o_numero_grande_atravessa_como_texto() {
    let dir = DirTemp::novo("seqn-grande");
    let (s, _) = servidor(&dir, cadastro("{}"));
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"nf","inicio":"9007199254740993"}"#,
    )
    .unwrap();
    let r = dono(
        &s,
        "proximo_da_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#,
    )
    .unwrap();
    assert_eq!(r.campo("valor"), Some(&Json::texto_de("9007199254740993")));
    // E o numero cru nessa faixa e recusado na entrada, dizendo o campo.
    let e = dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"g","inicio":9007199254740993}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("inicio"), "{e}");
    assert!(e.to_string().contains("perde precisao"), "{e}");
}

/// Esgotada sem ciclo e erro lido; com ciclo, volta.
#[test]
fn esgotar_e_erro_e_o_ciclo_volta() {
    let dir = DirTemp::novo("seqn-esgota");
    let (s, _) = servidor(&dir, cadastro("{}"));
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"nf","maximo":2}"#,
    )
    .unwrap();
    for _ in 0..2 {
        dono(
            &s,
            "proximo_da_sequencia",
            r#"{"database":"b","sequencia":"nf"}"#,
        )
        .unwrap();
    }
    let e = dono(
        &s,
        "proximo_da_sequencia",
        r#"{"database":"b","sequencia":"nf"}"#,
    )
    .unwrap_err();
    assert!(matches!(e, PhxError::LimiteExcedido(_)), "{e}");
    assert!(
        dono(&s, "sequencia", r#"{"database":"b","sequencia":"nf"}"#)
            .unwrap()
            .booleano_ou("esgotada", false)
    );
    dono(
        &s,
        "criar_sequencia",
        r#"{"database":"b","sequencia":"c","maximo":2,"ciclo":true}"#,
    )
    .unwrap();
    let valores: Vec<i64> = (0..3)
        .map(|_| {
            dono(
                &s,
                "proximo_da_sequencia",
                r#"{"database":"b","sequencia":"c"}"#,
            )
            .unwrap()
            .inteiro_ou("valor", -1)
        })
        .collect();
    assert_eq!(valores, vec![1, 2, 1]);
}
