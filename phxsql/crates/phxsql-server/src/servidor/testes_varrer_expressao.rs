//! O filtro por EXPRESSAO, no `varrer` e no `SelectMemory`.
//!
//! # O que ele acrescenta ao `"onde"`, e por que nao substitui
//!
//! `Filtro{coluna, op, valor}` compara UMA coluna com UM literal. Nao ha como
//! escrever `preco * 1.1 > 100`, nem `upper(cidade) = 'BLUMENAU'`, nem
//! `a > b`. A expressao escreve, e ela e a MESMA gramatica do `CHECK`, do
//! `DEFAULT`, da coluna calculada e do indice parcial -- uma so, no
//! `phxsql_core::expressao`, porque duas divergiriam no dia em que alguem
//! ensinasse `LIKE` a uma delas.
//!
//! O `"onde"` fica, e nao e legado: ele e o unico dos dois que o motor sabe
//! atalhar por mapa de igualdade (ver `SelectMemory`), e um cliente que monta
//! filtro por tela monta objeto, nao frase.
use super::*;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("varrer-expr-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Cinco linhas, uma delas com `cidade` NULA -- porque o `NULL` e o unico
/// ponto em que um filtro por expressao pode divergir do SQL calado.
fn servidor_com_precos(d: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        max_linhas: 10_000,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let ses = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","tabela":"precos","colunas":[
                    {"nome":"id","tipo":"Int8","obrigatoria":true},
                    {"nome":"cidade","tipo":"Str(30)"},
                    {"nome":"preco","tipo":"Decimal(12,2)"}],
                 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    for (id, cidade, preco) in [
        (1, r#""Blumenau""#, "10.00"),
        (2, r#""Itajai""#, "100.00"),
        (3, r#""Blumenau""#, "91.00"),
        (4, r#""Joinville""#, "1000.00"),
        (5, "null", "50.00"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"loja","tabela":"precos",
                         "linha":{{"id":{id},"cidade":{cidade},"preco":"{preco}"}}}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    s
}

fn varrer(s: &Arc<Servidor>, extra: &str) -> Result<Json> {
    s.executar(
        "varrer",
        &pedido(&format!(
            r#"{{"database":"loja","tabela":"precos","max":100{extra}}}"#
        )),
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

/// **O teste que mais importa, e e o do comportamento VELHO.**
///
/// Guarda nova entra pedida, nao imposta: quem nunca ouviu falar de
/// `"expressao"` recebe a mesma pagina, com os mesmos cursores e os mesmos
/// contadores. Se este cair, a funcionalidade nova tirou algo de quem nao
/// pediu nada.
#[test]
fn sem_expressao_nada_muda() {
    let d = dir("velho");
    let s = servidor_com_precos(&d);
    let r = varrer(&s, "").unwrap();
    assert_eq!(ids(&r), vec![1, 2, 3, 4, 5]);
    assert_eq!(r.inteiro_ou("devolvidas", -1), 5);
    assert_eq!(r.inteiro_ou("examinadas", -1), 5);
    assert_eq!(r.inteiro_ou("visiveis", -1), 5);
    // E a expressao VAZIA e o mesmo que expressao nenhuma: um cliente que
    // monta o campo sempre e manda "" nao pode receber zero linha.
    let r = varrer(&s, r#","expressao":"""#).expect("expressao vazia tinha de passar");
    assert_eq!(r.inteiro_ou("devolvidas", -1), 5);
    let _ = std::fs::remove_dir_all(&d);
}

/// **A PROVA REAL, e ela mede QUANTO veio -- nao se filtrou.**
///
/// Com a peneira desligada (o `continue` do `varrer_a_pagina` fora, ou o
/// `expressao.as_ref()` trocado por `None`), esta primeira assercao volta
/// a ver 5 e o teste REPROVA. Um teste que so perguntasse «todas passam na
/// conta?» passaria com o defeito reposto se o cliente peneirasse depois
/// -- e teste que passa por engano e pior que teste que falta.
///
/// A conta e DECIMAL EXATO: `91.00 * 1.1` e `100.10`, e nao `100.1000...`
/// de um `f64`. Por isso a linha 3 entra e a 2 (`100.00 * 1.1 = 110.00`)
/// tambem, enquanto uma implementacao que arredondasse antes de comparar
/// erraria justamente na linha 3.
#[test]
fn a_expressao_filtra_e_a_conta_e_exata() {
    let d = dir("conta");
    let s = servidor_com_precos(&d);
    let r = varrer(&s, r#","expressao":"preco * 1.1 > 100""#).unwrap();
    assert_eq!(
        r.inteiro_ou("devolvidas", -1),
        3,
        "o servidor mandou o que a tela ia jogar fora"
    );
    assert_eq!(ids(&r), vec![2, 3, 4]);
    // O que a peneira NAO remove: a varredura continua olhando as cinco.
    assert_eq!(r.inteiro_ou("examinadas", -1), 5);
    assert_eq!(r.inteiro_ou("visiveis", -1), 5);
    let _ = std::fs::remove_dir_all(&d);
}

/// `NULL` EXCLUI num filtro, e essa e a decisao que o contrato fixa.
///
/// A linha 5 tem `cidade` nula. `cidade <> 'Itajai'` da `NULL` para ela --
/// e nao `TRUE`, como quem le depressa esperaria. Ela sai, como sai em
/// qualquer SQL. E `IS NULL` e o unico que responde sobre ela.
#[test]
fn nulo_exclui_e_so_o_is_null_o_ve() {
    let d = dir("nulo");
    let s = servidor_com_precos(&d);
    let r = varrer(&s, r#","expressao":"cidade <> 'Itajai'""#).unwrap();
    assert_eq!(ids(&r), vec![1, 3, 4], "o nulo entrou no <>");
    let r = varrer(&s, r#","expressao":"cidade IS NULL""#).unwrap();
    assert_eq!(ids(&r), vec![5]);
    let _ = std::fs::remove_dir_all(&d);
}

/// As duas metades do predicado valem JUNTAS, e nao uma ou outra.
///
/// O `"onde"` e a `"expressao"` no mesmo pedido tem de se somar (E). Se
/// alguem trocar o lugar unico por dois lugares, o mais provavel e um
/// deles vencer -- e o numero abaixo acusa qual.
#[test]
fn onde_e_expressao_valem_juntos() {
    let d = dir("juntos");
    let s = servidor_com_precos(&d);
    let r = varrer(
        &s,
        r#","onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"}],
               "expressao":"preco > 50""#,
    )
    .unwrap();
    assert_eq!(ids(&r), vec![3], "as duas metades tem de valer juntas");
    let _ = std::fs::remove_dir_all(&d);
}

/// Coluna inventada recusa na ANALISE, nomeando -- e nao devolve zero
/// linha, que e a resposta que parece certa.
#[test]
fn coluna_inventada_na_expressao_recusa_nomeando() {
    let d = dir("coluna");
    let s = servidor_com_precos(&d);
    let e = varrer(&s, r#","expressao":"cidde = 'Blumenau'""#).unwrap_err();
    assert!(e.to_string().contains("cidde"), "{e}");
    assert!(e.to_string().contains("precos"), "{e}");
    // E sintaxe quebrada tambem recusa, em vez de virar «nenhuma linha».
    let e = varrer(&s, r#","expressao":"preco >""#).expect_err("truncada tinha de recusar");
    assert!(!e.to_string().is_empty(), "recusa muda nao ensina nada");
    let _ = std::fs::remove_dir_all(&d);
}

/// A MESMA expressao tem de dar a MESMA resposta na memoria e no disco.
///
/// E a guarda contra a divergencia que duas copias do predicado teriam
/// criado: o `varrer` e o `SelectMemory` chamam `memoria::passa`, e este
/// teste e o que acusa se um dia alguem escrever a segunda.
#[test]
fn a_expressao_do_varrer_e_a_do_selectmemory() {
    let d = dir("gemeos");
    let s = servidor_com_precos(&d);
    let ses = Sessao::default();
    s.executar(
        "memoria_carregar",
        &pedido(r#"{"database":"loja","tabela":"precos"}"#),
        &ses,
    )
    .unwrap();
    for expr in [
        "preco * 1.1 > 100",
        "cidade IS NULL",
        "upper(cidade) = 'BLUMENAU'",
        "id BETWEEN 2 AND 4",
        "cidade LIKE 'B%'",
        "preco > 10 AND preco < 1000",
    ] {
        let disco = varrer(
            &s,
            &format!(r#","expressao":{}"#, Json::texto_de(expr).escrever()),
        )
        .unwrap_or_else(|e| panic!("{expr}: {e}"));
        let memoria = s
            .executar(
                "SelectMemory",
                &pedido(&format!(
                    r#"{{"database":"loja","tabela":"precos","max":100,"expressao":{}}}"#,
                    Json::texto_de(expr).escrever()
                )),
                &ses,
            )
            .unwrap_or_else(|e| panic!("{expr}: {e}"));
        assert_eq!(
            ids(&disco),
            ids(&memoria),
            "{expr}: o disco e a memoria discordaram"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A expressao entra na TRILHA de acesso, junto do `onde`.
///
/// Trilha que guarda metade do criterio nao responde a pergunta inteira --
/// e a pergunta e «quem procurou o que?».
#[test]
fn o_criterio_da_trilha_guarda_a_expressao() {
    let d = dir("trilha");
    let s = servidor_com_precos(&d);
    let ses = Sessao::default();
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"loja","tabela":"precos","colunas":{"cidade":"pessoal"}}"#),
        &ses,
    )
    .unwrap();
    varrer(&s, r#","expressao":"preco > 50""#).unwrap();
    let t = s
        .executar(
            "trilha",
            &pedido(r#"{"database":"loja","tabela":"precos"}"#),
            &ses,
        )
        .unwrap();
    let texto = t.escrever();
    assert!(
        texto.contains("expressao=") && texto.contains("preco > 50"),
        "a trilha nao guardou a expressao: {texto}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// E pelo `despachar`, que e por onde o pedido entra de verdade: o portao
/// da tabela continua valendo, e a expressao NAO nomeia tabela nenhuma --
/// ela fala de coluna da tabela que o portao ja leu.
#[test]
fn a_expressao_nao_contorna_o_portao_da_tabela() {
    let d = dir("portao");
    let s = servidor_com_precos(&d);
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"varrer","database":"loja","tabela":"precos",
                "expressao":"preco > 50"}"#,
        &mut ses,
        "127.0.0.1",
    );
    assert_eq!(ids(&r.unwrap()), vec![2, 3, 4]);
    let _ = std::fs::remove_dir_all(&d);
}
