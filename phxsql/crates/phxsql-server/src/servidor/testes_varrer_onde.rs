//! O `WHERE` do `varrer`: o predicado que o protocolo nao tinha.
//!
//! # Por que ele existe, e por que ele para onde para
//!
//! A grade da tela filtra o que esta NELA: pedia `varrer max=2500`, recebia
//! 2.500 linhas e jogava fora 2.475 no navegador. Medido em
//! `--example onde-doi-no-varrer` numa tabela de 100.000 linhas, com uma em
//! cem casando: a leitura das linhas e **48,0%** do tempo e o transporte
//! (montar o JSON, serializar, o fio e a analise no cliente) e **52,0%**. O
//! teto previsto era **2,06x** e o medido pelo fio deu **2,07x** -- a
//! premissa se sustentou. Com metade da tabela casando, **1,35x**.
//!
//! `max` continua querendo dizer LINHAS EXAMINADAS, e nao devolvidas. Trocar
//! isso faria um filtro pouco seletivo varrer a tabela inteira com a trava
//! global na mao -- um custo que ninguem mediu.
use super::*;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("varrer-onde-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `n` linhas, uma em cada cem de Blumenau -- a seletividade da tela.
fn servidor_com_clientes(d: &std::path::Path, n: i64) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        max_linhas: 100_000,
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","tabela":"clientes","colunas":[
                    {"nome":"id","tipo":"Int8","obrigatoria":true},
                    {"nome":"cidade","tipo":"Str(30)"}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    let linhas: Vec<String> = (1..=n)
        .map(|i| {
            let cidade = if i % 100 == 0 { "Blumenau" } else { "Itajai" };
            format!(r#"{{"id":{i},"cidade":"{cidade}"}}"#)
        })
        .collect();
    s.executar(
        "inserir_lote",
        &pedido(&format!(
            r#"{{"database":"loja","tabela":"clientes","linhas":[{}]}}"#,
            linhas.join(",")
        )),
        &sessao,
    )
    .unwrap();
    s
}

fn varrer(s: &Arc<Servidor>, extra: &str) -> Json {
    s.executar(
        "varrer",
        &pedido(&format!(
            r#"{{"database":"loja","tabela":"clientes"{extra}}}"#
        )),
        &Sessao::default(),
    )
    .unwrap()
}

/// **O teste que mais importa**, e e o do comportamento VELHO.
///
/// Guarda nova entra pedida, nao imposta: quem nunca ouviu falar de
/// `"onde"` -- todo cliente escrito antes desta versao -- recebe a mesma
/// pagina, do mesmo tamanho, com os mesmos cursores. Se este teste
/// quebrar, a funcionalidade nova tirou algo de quem nao pediu nada.
#[test]
fn sem_onde_nada_muda() {
    let d = dir("velho");
    let s = servidor_com_clientes(&d, 2500);

    let r = varrer(&s, r#","max":2500"#);
    assert_eq!(r.inteiro_ou("devolvidas", -1), 2500);
    // `examinadas` e campo NOVO, e sem filtro ele repete `devolvidas`:
    // e a verdade de sempre, agora dita em voz alta.
    assert_eq!(r.inteiro_ou("examinadas", -1), 2500);
    assert_eq!(r.inteiro_ou("visiveis", -1), 2500);
    assert_eq!(r.inteiro_ou("cursor_inicio", -1), 1);
    assert_eq!(r.inteiro_ou("cursor_fim", -1), 2500);
    assert!(!r.booleano_ou("ha_mais", true));

    // E a pagina curta continua curta.
    let r = varrer(&s, r#","max":10"#);
    assert_eq!(r.inteiro_ou("devolvidas", -1), 10);
    assert_eq!(r.inteiro_ou("examinadas", -1), 10);
    assert!(r.booleano_ou("ha_mais", false));

    let _ = std::fs::remove_dir_all(&d);
}

/// **A PROVA REAL, e ela mede QUANTO veio -- nao se filtrou.**
///
/// Com o predicado desligado (o `continue` do `op_varrer` fora), a busca
/// por Blumenau volta a trazer 2.500 e este teste REPROVA na primeira
/// assercao. Um teste que so perguntasse "todas sao de Blumenau?" passaria
/// com o defeito reposto se o cliente peneirasse depois -- e teste que
/// passa por engano e pior que teste que falta.
#[test]
fn blumenau_volta_25_e_nao_2500() {
    let d = dir("blumenau");
    let s = servidor_com_clientes(&d, 2500);

    let r = varrer(
        &s,
        r#","max":2500,"onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"}]"#,
    );
    assert_eq!(
        r.inteiro_ou("devolvidas", -1),
        25,
        "o servidor mandou o que a tela ia jogar fora"
    );
    // O que o filtro NAO remove: a varredura continua olhando as 2.500.
    // E o numero que impede a tela de mentir dizendo «a tabela tem 25».
    assert_eq!(r.inteiro_ou("examinadas", -1), 2500);
    assert_eq!(r.inteiro_ou("visiveis", -1), 2500);

    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas.len(), 25);
    for l in linhas {
        assert_eq!(l.texto_ou("cidade", ""), "Blumenau");
        assert_eq!(l.inteiro_ou("id", -1) % 100, 0);
    }

    let _ = std::fs::remove_dir_all(&d);
}

/// O mesmo filtro tem de dar a MESMA resposta na memoria e no disco.
///
/// A guarda contra a divergencia que duas copias de `casa` teriam criado:
/// o `varrer` e o `SelectMemory` chamam a mesma funcao do `phxsql-store`,
/// e este teste e o que acusa se um dia alguem escrever a segunda.
#[test]
fn o_filtro_do_varrer_e_o_do_selectmemory() {
    let d = dir("gemeos");
    let s = servidor_com_clientes(&d, 500);
    let sessao = Sessao::default();
    s.executar(
        "memoria_carregar",
        &pedido(r#"{"database":"loja","tabela":"clientes"}"#),
        &sessao,
    )
    .unwrap();

    // Um de cada familia de operador: igualdade, faixa, texto e nulo.
    for onde in [
        r#"[{"coluna":"cidade","op":"=","valor":"Blumenau"}]"#,
        r#"[{"coluna":"cidade","op":"contem","valor":"blu"}]"#,
        r#"[{"coluna":"id","op":">","valor":400}]"#,
        r#"[{"coluna":"id","op":"<=","valor":50},{"coluna":"cidade","op":"!=","valor":"Blumenau"}]"#,
        r#"[{"coluna":"cidade","op":"nulo"}]"#,
        r#"[{"coluna":"cidade","op":"nao_nulo"}]"#,
    ] {
        let disco = varrer(&s, &format!(r#","max":500,"onde":{onde}"#));
        let memoria = s
            .executar(
                "SelectMemory",
                &pedido(&format!(
                    r#"{{"database":"loja","tabela":"clientes","max":500,"onde":{onde}}}"#
                )),
                &sessao,
            )
            .unwrap();
        let ids = |j: &Json| -> Vec<i64> {
            j.campo("linhas")
                .and_then(Json::lista)
                .unwrap()
                .iter()
                .map(|l| l.inteiro_ou("rowid", -1))
                .collect()
        };
        assert_eq!(
            ids(&disco),
            ids(&memoria),
            "o filtro {onde} respondeu diferente no disco e na memoria"
        );
    }

    let _ = std::fs::remove_dir_all(&d);
}

/// O cursor e o ultimo rowid EXAMINADO, e nao o ultimo devolvido.
///
/// Se ele fosse o ultimo devolvido, a pagina seguinte comecaria depois da
/// ultima linha que CASOU -- e as linhas entre ela e o fim da pagina
/// examinada sumiriam sem ninguem ver. Aqui a soma das paginas tem de dar
/// o mesmo que a varredura inteira.
#[test]
fn paginar_com_filtro_nao_perde_linha() {
    let d = dir("cursor");
    let s = servidor_com_clientes(&d, 1000);
    let onde = r#""onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"}]"#;

    let inteiro = varrer(&s, &format!(r#","max":1000,{onde}"#));
    assert_eq!(inteiro.inteiro_ou("devolvidas", -1), 10);

    let mut juntas = Vec::new();
    let mut cursor = 0i64;
    loop {
        let r = varrer(&s, &format!(r#","max":300,"depois":{cursor},{onde}"#));
        for l in r.campo("linhas").and_then(Json::lista).unwrap() {
            juntas.push(l.inteiro_ou("rowid", -1));
        }
        if !r.booleano_ou("ha_mais", false) {
            break;
        }
        let novo = r.inteiro_ou("cursor_fim", -1);
        assert!(novo > cursor, "o cursor nao andou: {novo} <= {cursor}");
        cursor = novo;
    }
    let esperadas: Vec<i64> = (1..=10).map(|i| i * 100).collect();
    assert_eq!(juntas, esperadas, "paginar com filtro perdeu linha");

    let _ = std::fs::remove_dir_all(&d);
}

/// Coluna que nao existe e recusada na hora, com o nome dela no recado.
#[test]
fn coluna_inventada_no_onde_e_recusada() {
    let d = dir("coluna");
    let s = servidor_com_clientes(&d, 10);
    let e = s
        .executar(
            "varrer",
            &pedido(
                r#"{"database":"loja","tabela":"clientes",
                        "onde":[{"coluna":"bairro","op":"=","valor":"x"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("bairro"), "{e}");

    // E operador inventado tambem, com a lista dos que valem.
    let e = s
        .executar(
            "varrer",
            &pedido(
                r#"{"database":"loja","tabela":"clientes",
                        "onde":[{"coluna":"cidade","op":"~=","valor":"x"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("contem"), "{e}");

    let _ = std::fs::remove_dir_all(&d);
}

/// O filtro entra na TRILHA de acesso.
///
/// «Quem procurou os clientes de Blumenau?» e exatamente a pergunta que se
/// faz a uma trilha, e um criterio que dissesse so «varrer» responderia
/// menos do que respondia antes de o filtro existir.
#[test]
fn o_criterio_da_trilha_guarda_o_filtro() {
    let onde = vec![Filtro {
        coluna: 1,
        op: Operador::Igual,
        valor: Value::Str("Blumenau".into()),
    }];
    let esquema = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("cidade", ColumnType::Str(30)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    assert_eq!(
        descrever_filtros(&onde, &esquema),
        "cidade = Blumenau",
        "a trilha nao diz o que foi procurado"
    );
    assert_eq!(descrever_filtros(&[], &esquema), "");
}
