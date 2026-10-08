//! Pedido 679 (325 F3): a visao da loja no central, pelo `unir` sobre os
//! `caixaNN`, SEM juntar as fontes numa tabela.
//!
//! # Por que nao ha codigo novo aqui
//!
//! O desenho espelho (decisao do dono na linha do 325) poe um database por
//! caixa no central, cada um com um escritor so. O `unir` com `partes` ja
//! aceita um `database` por braco (`linhas_do_sub_pedido` herda o de fora so
//! quando o braco nao diz o dele), e cada braco passa pelo portao de
//! permissao como um pedido proprio. A loja se le empilhando os caixas, e a
//! ordem de digitacao de cada um fica no `.reg` dele.
//!
//! # O teto, e o que acontece acima dele
//!
//! Cada braco e um pedido inteiro em memoria, recortado por `max_linhas` (o
//! do topo do `config.json`, 1.000 de fabrica). O braco `varrer` de um caixa
//! com mais vendas que o teto vem CORTADO, e a uniao responde
//! `truncado: true` -- nao recusa, e nao mente que veio inteira. O braco
//! `agrupar` varre a tabela INTEIRA e devolve um grupo por caixa: e a forma
//! da visao da loja que vale acima do teto. A uniao em si para no `max` dela
//! (5.000.000, `TETO_PIVOT`).
//!
//! Os dois sentidos estao aqui: o `varrer` cortado diz que cortou, e o
//! `agrupar` sobre o mesmo caixa conta as seis vendas acima do teto de cinco.

mod comum;
use comum::{pedir, DirTemp};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "visao-da-loja";
const TETO: u64 = 5;

/// `(caixa, vendas)`: o terceiro passa do teto de linhas.
const CAIXAS: [(&str, i64); 3] = [("caixa01", 4), ("caixa02", 2), ("caixa03", 6)];

fn subir(base: &std::path::Path) -> u16 {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        max_linhas: TETO,
        ..Default::default()
    };
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    std::mem::forget(s);
    porta
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
    let r = pedir(porta, &format!("{{\"token\":\"{TOKEN}\",{corpo}}}"));
    let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
    assert!(j.booleano_ou("ok", false), "{corpo} -> {r}");
    j.campo("resultado").cloned().unwrap()
}

/// Cada caixa com a SUA tabela `vendas` -- o que a replicacao espelho poe no
/// central, uma origem por database.
fn semear(porta: u16) {
    for (caixa, n) in CAIXAS {
        exigir(
            porta,
            &format!(r#""op":"criar_database","database":"{caixa}""#),
        );
        exigir(
            porta,
            &format!(
                r#""op":"criar_tabela","database":"{caixa}","tabela":"vendas",
                   "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                              {{"nome":"valor","tipo":"Int8"}}],
                   "indices":[{{"nome":"pk","colunas":["id"],"unico":true,"primario":true}}]"#
            ),
        );
        for id in 1..=n {
            exigir(
                porta,
                &format!(
                    r#""op":"inserir","database":"{caixa}","tabela":"vendas",
                       "valores":{{"id":{id},"valor":{}}}"#,
                    id * 10
                ),
            );
        }
    }
}

/// Os bracos da uniao, um por caixa, com `braco` montando o pedido.
fn partes(braco: impl Fn(&str) -> String) -> String {
    CAIXAS
        .iter()
        .map(|(c, _)| braco(c))
        .collect::<Vec<_>>()
        .join(",")
}

fn numeros(j: &Json, campo: &str) -> Vec<i64> {
    j.campo(campo)
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|x| x.inteiro().unwrap_or(-1))
        .collect()
}

#[test]
fn a_loja_se_le_pelos_tres_caixas_sem_juntar_as_fontes() {
    let base = DirTemp::novo("visao-da-loja");
    std::fs::create_dir_all(&base.0).unwrap();
    let porta = subir(&base.0);
    semear(porta);

    // 1. Por grupo: uma linha por caixa, contagem e soma EXATAS -- inclusive
    //    no caixa03, que tem mais vendas que o teto.
    let r = exigir(
        porta,
        &format!(
            r#""op":"unir","database":"caixa01","modo":"tudo","partes":[{}]"#,
            partes(|c| format!(
                r#"{{"op":"agrupar","database":"{c}","tabela":"vendas",
                     "agregados":[{{"funcao":"contagem","apelido":"n"}},
                                  {{"funcao":"soma","coluna":"valor","apelido":"total"}}]}}"#
            ))
        ),
    );
    let linhas: Vec<Vec<i64>> = r
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| {
            l.lista()
                .unwrap()
                .iter()
                .map(|v| v.numero().map_or(-1, |n| n as i64))
                .collect()
        })
        .collect();
    assert_eq!(
        linhas,
        vec![vec![4, 100], vec![2, 30], vec![6, 210]],
        "a visao da loja por caixa nao bateu: {}",
        r.escrever()
    );
    assert!(!r.booleano_ou("truncado", true), "{}", r.escrever());
    assert_eq!(numeros(&r, "por_parte"), vec![1, 1, 1]);

    // 2. Linha a linha: o caixa03 passa do teto, e a uniao DIZ que cortou --
    //    cinco do caixa03, e nao seis anunciadas como a loja inteira.
    let r = exigir(
        porta,
        &format!(
            r#""op":"unir","database":"caixa01","modo":"tudo","partes":[{}]"#,
            partes(|c| format!(r#"{{"op":"varrer","database":"{c}","tabela":"vendas"}}"#))
        ),
    );
    assert_eq!(numeros(&r, "por_parte"), vec![4, 2, TETO as i64]);
    assert!(
        r.booleano_ou("truncado", false),
        "o braco do caixa03 parou no teto e a uniao respondeu como se fosse a loja \
         inteira: {}",
        r.escrever()
    );

    // 3. As fontes ficam onde estavam: a uniao nao criou database nem
    //    tabela nenhuma -- no disco do central so os tres caixas.
    let mut dbs: Vec<String> = std::fs::read_dir(&base.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with("phx"))
        .collect();
    dbs.sort_unstable();
    assert_eq!(dbs, vec!["caixa01", "caixa02", "caixa03"]);
    for (caixa, _) in CAIXAS {
        let tabelas: Vec<String> = std::fs::read_dir(base.0.join(caixa))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".reg"))
            .collect();
        assert_eq!(tabelas, vec!["vendas.reg".to_string()], "{caixa}");
    }
}
