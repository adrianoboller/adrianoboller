//! A janela de conflito de escrita, pelo protocolo.
//!
//! O que estes testes travam nao e so o "recusa quando a versao e velha" --
//! e principalmente o **contrario**: o cliente que nao manda versao nenhuma
//! tem de continuar gravando como sempre gravou. Uma guarda que quebra todo
//! cliente antigo nao e protecao, e um estrago.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("conf-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um banco com uma tabela de uma linha, e a sessao para mexer nela.
fn com_uma_linha(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"c","linha":{"id":1,"nome":"Adriano"}}"#),
        &sessao,
    )
    .unwrap();
    s
}

/// Quem nao pede versao recebe a linha crua, como sempre recebeu.
#[test]
fn ler_sem_pedir_versao_nao_muda_de_forma() {
    let dir = dir_temp("forma");
    let s = com_uma_linha(&dir);
    let r = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.texto_ou("nome", ""), "Adriano");
    assert!(r.campo("versao").is_none(), "a versao vazou na linha crua");
    assert!(r.campo("linha").is_none(), "a forma da resposta mudou");
}

#[test]
fn ler_com_versao_devolve_a_linha_e_a_versao() {
    let dir = dir_temp("com-versao");
    let s = com_uma_linha(&dir);
    let r = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1,"com_versao":true}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("versao", -1), 1);
    assert_eq!(r.inteiro_ou("rowid", -1), 1);
    assert_eq!(
        r.campo("linha").unwrap().texto_ou("nome", ""),
        "Adriano",
        "a linha nao veio dentro do envelope"
    );
}

/// O cliente antigo -- o que nao sabe o que e versao -- continua gravando.
#[test]
fn atualizar_sem_versao_continua_gravando() {
    let dir = dir_temp("antigo");
    let s = com_uma_linha(&dir);
    let sessao = Sessao::default();
    for nome in ["Maria", "Joao"] {
        s.executar(
            "atualizar",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","rowid":1,"linha":{{"id":1,"nome":"{nome}"}}}}"#
            )),
            &sessao,
        )
        .unwrap();
    }
    let r = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.texto_ou("nome", ""), "Joao");
}

/// A resposta do `atualizar` traz a versao nova: quem grava duas vezes
/// seguidas nao precisa reler a linha inteira no meio.
#[test]
fn atualizar_devolve_a_versao_nova() {
    let dir = dir_temp("devolve");
    let s = com_uma_linha(&dir);
    let r = s
        .executar(
            "atualizar",
            &pedido(
                r#"{"database":"b","tabela":"c","rowid":1,
                        "linha":{"id":1,"nome":"Maria"},"versao":1}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("versao", -1), 2);
}

/// Os dois leem a versao 1; o segundo chega depois e e recusado.
#[test]
fn atualizar_com_versao_velha_recusa() {
    let dir = dir_temp("velha");
    let s = com_uma_linha(&dir);
    let sessao = Sessao::default();
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"b","tabela":"c","rowid":1,
                    "linha":{"id":1,"nome":"Maria"},"versao":1}"#,
        ),
        &sessao,
    )
    .unwrap();

    let e = s
        .executar(
            "atualizar",
            &pedido(
                r#"{"database":"b","tabela":"c","rowid":1,
                        "linha":{"id":1,"nome":"Joao"},"versao":1}"#,
            ),
            &sessao,
        )
        .unwrap_err();
    assert_eq!(e.codigo(), 3004);
    assert_eq!(e.nome(), "CONFLITO");

    // E nada foi gravado: o trabalho do primeiro esta inteiro.
    let r = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1,"com_versao":true}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.campo("linha").unwrap().texto_ou("nome", ""), "Maria");
    assert_eq!(r.inteiro_ou("versao", -1), 2);
}

/// Excluir uma linha que outra pessoa acabou de alterar e a mesma janela.
#[test]
fn excluir_com_versao_velha_recusa() {
    let dir = dir_temp("excluir");
    let s = com_uma_linha(&dir);
    let sessao = Sessao::default();
    s.executar(
        "atualizar",
        &pedido(r#"{"database":"b","tabela":"c","rowid":1,"linha":{"id":1,"nome":"Maria"}}"#),
        &sessao,
    )
    .unwrap();

    let e = s
        .executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1,"versao":1}"#),
            &sessao,
        )
        .unwrap_err();
    assert_eq!(e.nome(), "CONFLITO");

    // A linha continua la, e nao marcada.
    let r = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1);
}

/// Zero e ausente sao a mesma coisa. Sem isto, um cliente que guarda a
/// versao num campo numerico nao inicializado gravaria sempre -- ou nunca.
#[test]
fn versao_zero_e_o_mesmo_que_nao_mandar() {
    let dir = dir_temp("zero");
    let s = com_uma_linha(&dir);
    let sessao = Sessao::default();
    s.executar(
        "atualizar",
        &pedido(r#"{"database":"b","tabela":"c","rowid":1,"linha":{"id":1,"nome":"Maria"}}"#),
        &sessao,
    )
    .unwrap();
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"b","tabela":"c","rowid":1,
                    "linha":{"id":1,"nome":"Joao"},"versao":0}"#,
        ),
        &sessao,
    )
    .unwrap();
}
