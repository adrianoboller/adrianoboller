//! O medidor da catraca do corpo legitimo do pedido 495 (fatia F1).
//!
//! `cargo run --example sinais-no-legitimo -p phxsql-sql -- LEGITIMO DETECCAO`
//!
//! Os dois `.jsonl` saem dos extratores de `bancada/seguranca/495/`, e quem
//! julga e o `catraca_sinais.py` de la: este binario so conta. Imprime cada
//! comando legitimo acusado (origem, classes, texto) e fecha com duas linhas
//! de maquina, `legitimo_acusados=N` e `ataques_acusados=A de=B`.
//!
//! # O que sai do corpo legitimo, e por que
//!
//! O extrator pega TODO literal de SQL do repositorio -- inclusive os ataques
//! que os testes escrevem para provar que acusam. Contar esses como falso
//! positivo poria na catraca o proprio caso de prova. Saem dois grupos, e
//! nenhum por lista digitada aqui: o texto que o `extrair_deteccao.py` ja
//! rotulou (o `ARSENAL` e os ataques do `comando_empilhado`), e o que mora
//! no `sinais.rs`, cujos testes sao, por desenho, o corpo de ataque do
//! proprio detector.

use std::collections::HashSet;

use phxsql_core::json::Json;

/// O arquivo dos testes do detector: tudo o que ele escreve e ataque.
const ORIGEM_DO_DETECTOR: &str = "crates/phxsql-sql/src/sinais.rs";

fn ler(caminho: &str) -> Vec<Json> {
    let texto = std::fs::read_to_string(caminho)
        .unwrap_or_else(|e| panic!("nao li {caminho}: {e} -- rode os extratores antes"));
    texto
        .lines()
        .map(|l| Json::analisar(l).unwrap_or_else(|e| panic!("{caminho}: {e}")))
        .collect()
}

fn acusa(sql: &str) -> Option<phxsql_sql::Sinais> {
    let s = phxsql_sql::lexico::analisar_com_comentarios(sql).ok()?;
    Some(phxsql_sql::sinais(&s))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(legitimo), Some(deteccao)) = (args.next(), args.next()) else {
        eprintln!("uso: sinais-no-legitimo LEGITIMO.jsonl DETECCAO.jsonl");
        std::process::exit(2);
    };
    let deteccao = ler(&deteccao);
    let rotulados: HashSet<&str> = deteccao.iter().map(|j| j.texto_ou("sql", "")).collect();

    let (mut ataques, mut ataques_acusados) = (0, 0);
    for j in deteccao
        .iter()
        .filter(|j| j.texto_ou("rotulo", "") == "ataque")
    {
        ataques += 1;
        if acusa(j.texto_ou("sql", "")).is_some_and(|r| !r.vazio()) {
            ataques_acusados += 1;
        } else {
            println!("ATAQUE NAO ACUSADO\t{}", j.texto_ou("sql", ""));
        }
    }

    let (mut lidos, mut acusados) = (0, 0);
    for j in &ler(&legitimo) {
        let sql = j.texto_ou("sql", "");
        let origem = j.texto_ou("origem", "");
        // O `comando.sql` e um gerado de 20.000 linhas iguais na forma: ele
        // pesaria o corpo sem acrescentar forma nenhuma.
        if origem.starts_with("bancada/comando.sql")
            || origem.starts_with(ORIGEM_DO_DETECTOR)
            || rotulados.contains(sql)
        {
            continue;
        }
        let Some(r) = acusa(sql) else { continue };
        lidos += 1;
        if !r.vazio() {
            acusados += 1;
            println!(
                "{origem}\t{:?}\t{}",
                r.nomes().collect::<Vec<_>>(),
                sql.replace('\n', " ")
            );
        }
    }
    println!("legitimo_lidos={lidos}");
    println!("legitimo_acusados={acusados}");
    println!("ataques_acusados={ataques_acusados} de={ataques}");
}
