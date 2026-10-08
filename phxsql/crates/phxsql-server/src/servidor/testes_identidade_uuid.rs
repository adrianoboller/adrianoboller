//! **A identidade `Uuid` que nasce sozinha, PELO PROTOCOLO** -- §B.2.5 do
//! `docs/AUTONUMBER.md`.
//!
//! O teste do `store` prova o motor; este prova o caminho que o cliente usa,
//! que e' outro: `json_para_linha` transforma coluna AUSENTE em `Value::Null`
//! (`valores.rs`), e era exatamente ali que o pedido morria com «coluna id e
//! obrigatoria e recebeu NULL». O `"novo"` ja existia (`valores.rs`, ramo
//! `ColumnType::Uuid`); o que faltava era o caso de quem nao manda nada.
use super::*;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("iduuid-{nome}"))
}

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
    s.executar(
        "criar_database",
        &Json::analisar(r#"{"database":"b"}"#).unwrap(),
        &Sessao::default(),
    )
    .unwrap();
    s
}

/// Pelo `despachar`: e por onde o pedido entra de verdade.
fn pede(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

fn com_identidade(s: &Arc<Servidor>) -> Result<Json> {
    pede(
        s,
        r#""op":"criar_tabela","database":"b","tabela":"notas",
               "colunas":[{"nome":"id","tipo":"Uuid","obrigatoria":true},
                          {"nome":"texto","tipo":"Str(40)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    )
}

#[test]
fn quem_nao_manda_o_id_ganha_um() {
    let guarda = dir_temp("ganha");
    let s = servidor(&guarda);
    com_identidade(&s).unwrap();

    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"notas","linha":{"texto":"sem id"}"#,
    )
    .unwrap_or_else(|e| panic!("insercao sem id recusada: {e}"));

    let l = pede(
        &s,
        r#""op":"ler","database":"b","tabela":"notas","rowid":1"#,
    )
    .unwrap();
    let id = l.texto_ou("id", "");
    assert_eq!(id.len(), 36, "o id nao voltou como uuid: {}", l.escrever());
    let u = phxsql_core::uuid::Uuid::de_texto(id).expect("o id nao e um uuid");
    assert_eq!(u.versao(), 7, "a identidade tem de ser v7");
}

#[test]
fn id_nulo_explicito_tambem_ganha() {
    // Cliente que manda a ficha inteira com o campo vazio -- e' a tela,
    // e' o driver, e' o ODBC. O ausente e o nulo tem de sair no mesmo
    // lugar, senao a regra depende de como o pedido foi escrito.
    let guarda = dir_temp("nulo");
    let s = servidor(&guarda);
    com_identidade(&s).unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"notas","linha":{"id":null,"texto":"nulo"}"#,
    )
    .unwrap_or_else(|e| panic!("insercao com id nulo recusada: {e}"));
    let l = pede(
        &s,
        r#""op":"ler","database":"b","tabela":"notas","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.texto_ou("id", "").len(), 36, "{}", l.escrever());
}

#[test]
fn o_novo_continua_valendo_e_o_id_escolhido_tambem() {
    // O comportamento VELHO, nos dois modos que ja existiam: a palavra
    // `"novo"` e o id escrito por quem chama.
    let guarda = dir_temp("velho");
    let s = servidor(&guarda);
    com_identidade(&s).unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"notas","linha":{"id":"novo","texto":"a"}"#,
    )
    .unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"notas",
               "linha":{"id":"0191f0c0-0000-7000-8000-000000000001","texto":"b"}"#,
    )
    .unwrap();
    let a = pede(
        &s,
        r#""op":"ler","database":"b","tabela":"notas","rowid":1"#,
    )
    .unwrap();
    let b = pede(
        &s,
        r#""op":"ler","database":"b","tabela":"notas","rowid":2"#,
    )
    .unwrap();
    assert_eq!(a.texto_ou("id", "").len(), 36);
    assert_eq!(b.texto_ou("id", ""), "0191f0c0-0000-7000-8000-000000000001");
}

#[test]
fn sem_primaria_marcada_o_erro_velho_continua() {
    // A CATRACA DO COMPORTAMENTO VELHO. O indice e' `unico` e nao
    // `primario`: sem a identidade declarada nada muda, e quem usa esse
    // erro para pegar «esqueci de preencher» continua pegando.
    let guarda = dir_temp("sem-pk");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"notas",
               "colunas":[{"nome":"id","tipo":"Uuid","obrigatoria":true},
                          {"nome":"texto","tipo":"Str(40)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
    )
    .unwrap();
    let e = pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"notas","linha":{"texto":"sem id"}"#,
    )
    .expect_err("sem primaria marcada, o id nulo tinha de recusar");
    let txt = e.to_string();
    assert!(
        txt.contains("obrigatoria") && txt.contains("id"),
        "o erro velho mudou de texto: {txt}"
    );
}
