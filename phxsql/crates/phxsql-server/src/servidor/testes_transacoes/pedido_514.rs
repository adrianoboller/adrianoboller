//! Pedido 514 pelo servidor: a chave estrangeira confere a linha FINAL, e
//! o DEFAULT e a coluna calculada que o motor escreve nao passam por
//! baixo dela -- fora de transacao, no `inserir`, e dentro, no COMMIT, que
//! recusa ANTES da marca (a pre-conferencia do 448) com zero gravado. A
//! prova do `store` mora em `tests/fk-na-linha-final.rs`; esta prova o
//! caminho que o cliente usa.
use super::*;

/// `clientes(id, codigo)` com `codigo` unico e referenciado, e a filha
/// com a coluna da chave declarada como o teste pedir. A chave sobre
/// calculada declara `restringir`: desde o P1 do 514, calculada em
/// cascata se recusa na declaracao.
fn base(s: &Arc<Servidor>, ses: &Sessao, tabela: &str, colunas: &str) {
    pede(s, ses, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"codigo","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_codigo","colunas":["codigo"],"unico":true}]"#,
    )
    .unwrap();
    pede(
        s,
        ses,
        &format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
                       "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},{colunas}],
                       "indices":[{{"nome":"pk","colunas":["id"],"unico":true,"primario":true}},
                                  {{"nome":"por_cliente","colunas":["cod_cliente"]}}],
                       "chaves_estrangeiras":[{{"nome":"fk_cliente","colunas":["cod_cliente"],
                                               "tabela_ref":"clientes","colunas_ref":["codigo"],
                                               "ao_alterar":"{ao_alterar}"}}]"#,
            ao_alterar = if colunas.contains("calculada") {
                "restringir"
            } else {
                "cascata"
            }
        ),
    )
    .unwrap();
}

fn cliente(s: &Arc<Servidor>, ses: &Sessao, id: i64, codigo: i64) {
    pede(
        s,
        ses,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{id},"codigo":{codigo}}}"#
        ),
    )
    .unwrap();
}

fn inserir(s: &Arc<Servidor>, ses: &Sessao, tabela: &str, linha: &str) -> Result<Json> {
    pede(
        s,
        ses,
        &format!(r#""op":"inserir","database":"loja","tabela":"{tabela}","linha":{linha}"#),
    )
}

/// O COMMIT que tem de recusar: erro de INTEGRIDADE nomeando a chave,
/// e (linhas, slots, marcas) de `tabela` iguais aos de antes do BEGIN.
fn commit_recusa(
    s: &Arc<Servidor>,
    ses: &Sessao,
    dir: &std::path::Path,
    tabela: &str,
    antes: (u64, u64),
) {
    let r = pede(s, ses, r#""op":"commit""#);
    let depois = (quantas(s, ses, tabela), slots(s, ses, tabela));
    match r {
        Err(e) => {
            let texto = e.to_string();
            assert_eq!(e.nome(), "INTEGRIDADE", "{texto}");
            assert!(texto.contains("fk_cliente"), "{texto}");
            assert_eq!(depois, antes, "(linhas, slots): {texto}");
        }
        Ok(j) => panic!(
            "o COMMIT gravou a filha orfa: {} -- (linhas, slots) antes {antes:?}, \
                     depois {depois:?}",
            j.escrever()
        ),
    }
    assert_eq!(marcas_em(&dir.join("loja")), 0);
}

/// **O caso do parecer, N1:** `cod_cliente` pelo DEFAULT 7, sem
/// cliente 7. Gravava fora e dentro da transacao. E o controle: com o
/// cliente 7, os dois caminhos gravam -- e gravam o 7.
#[test]
fn o_default_sem_mae_e_recusado_fora_e_dentro_da_transacao() {
    let dir = dir_temp("514-default");
    let s = servidor(&dir);
    let ses = sessao(5141);
    base(
        &s,
        &ses,
        "pedidos",
        r#"{"nome":"cod_cliente","tipo":"Int8","padrao":7}"#,
    );
    cliente(&s, &ses, 1, 1);

    let fora = inserir(&s, &ses, "pedidos", r#"{"id":1}"#);
    match &fora {
        Err(e) => assert!(
            e.nome() == "INTEGRIDADE" && e.to_string().contains("fk_cliente"),
            "{e}"
        ),
        Ok(j) => panic!(
            "fora de transacao, o DEFAULT 7 sem mae gravou: {}",
            j.escrever()
        ),
    }
    assert_eq!(quantas(&s, &ses, "pedidos"), 0);

    let antes = (quantas(&s, &ses, "pedidos"), slots(&s, &ses, "pedidos"));
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    inserir(&s, &ses, "pedidos", r#"{"id":2}"#).unwrap();
    commit_recusa(&s, &ses, &dir, "pedidos", antes);

    // O controle positivo, pelos dois caminhos.
    cliente(&s, &ses, 7, 7);
    inserir(&s, &ses, "pedidos", r#"{"id":3}"#)
        .expect("o DEFAULT 7 com o cliente 7 foi recusado fora de transacao");
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    inserir(&s, &ses, "pedidos", r#"{"id":4}"#).unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#)
        .expect("o DEFAULT 7 com o cliente 7 foi recusado no COMMIT");
    assert_eq!(r.texto_ou("transaction_state", ""), "COMMITTED");
    let codigos: Vec<i64> = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"loja","tabela":"pedidos","max":10"#,
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .map(|l| l.campo("cod_cliente").and_then(Json::inteiro).unwrap_or(-1))
    .collect();
    assert_eq!(codigos, vec![7, 7]);
}

/// **A calculada do parecer, N1:** `cod_cliente = x + 0`. Inserir com
/// `x = 9` sem cliente 9 gravava; alterar para `x = 8` sem cliente 8,
/// dentro de transacao, tambem.
#[test]
fn a_calculada_sem_mae_e_recusada_fora_e_dentro_da_transacao() {
    let dir = dir_temp("514-calculada");
    let s = servidor(&dir);
    let ses = sessao(5142);
    base(
        &s,
        &ses,
        "itens",
        r#"{"nome":"cod_cliente","tipo":"Int8","calculada":"x + 0"},
                   {"nome":"x","tipo":"Int8"}"#,
    );
    cliente(&s, &ses, 1, 9);

    let fora = inserir(&s, &ses, "itens", r#"{"id":1,"x":8}"#);
    assert!(
        matches!(&fora, Err(e) if e.nome() == "INTEGRIDADE"),
        "fora de transacao, a calculada 8 sem mae: {:?}",
        fora.map(|j| j.escrever())
    );
    inserir(&s, &ses, "itens", r#"{"id":1,"x":9}"#)
        .expect("a calculada 9 com o cliente 9 foi recusada");

    let antes = (quantas(&s, &ses, "itens"), slots(&s, &ses, "itens"));
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"itens","rowid":1,
                   "linha":{"id":1,"cod_cliente":9,"x":8}"#,
    )
    .unwrap();
    commit_recusa(&s, &ses, &dir, "itens", antes);
    let linha = pede(
        &s,
        &ses,
        r#""op":"ler","database":"loja","tabela":"itens","rowid":1"#,
    )
    .unwrap();
    let linha = linha.campo("linha").unwrap_or(&linha);
    assert_eq!(
        (
            linha.campo("x").and_then(Json::inteiro),
            linha.campo("cod_cliente").and_then(Json::inteiro)
        ),
        (Some(9), Some(9)),
        "a alteracao recusada mudou a linha: {}",
        linha.escrever()
    );
}
