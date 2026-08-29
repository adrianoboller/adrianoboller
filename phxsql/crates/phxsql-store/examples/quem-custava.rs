//! Das duas coisas que o ponto de captura fazia com o profiler desligado --
//! dois `Json::analisar` do corpo e um mutex --, qual custava?
//!
//! ```bash
//! cargo run --release --example quem-custava
//! ```
//!
//! A pergunta nasceu de uma resposta minha exagerada: eu escrevi que "o mutex
//! era o pior pedaco, porque ele SERIALIZA". A segunda parte e verdade sobre
//! mutex em geral e nao era verdade AQUI -- e a diferenca entre as duas so
//! aparece medindo.
//!
//! Este medidor separa os dois. O que ele mostra e que a comparacao nem e
//! disputada: o `lock` sem disputa custa nanossegundos, e analisar o corpo de
//! um lote custa MILISSEGUNDOS.
//!
//! (E, no PhxSql, o mutex do profiler nunca foi o gargalo de concorrencia por
//! outro motivo: toda operacao de dado ja se serializa na trava global, que e
//! tomada DEPOIS e segurada por muito mais tempo.)

use std::sync::Mutex;
use std::time::Instant;

use phxsql_core::json::Json;

/// O corpo de um `inserir_lote`, como ele chega pela porta.
fn corpo(n: usize) -> String {
    let linhas: Vec<String> = (1..=n)
        .map(|i| format!(r#"{{"id":{i},"produto":"Produto {i:08}","cidade":"Blumenau"}}"#))
        .collect();
    format!(
        r#"{{"token":"t","op":"inserir_lote","database":"loja","tabela":"clientes","linhas":[{}]}}"#,
        linhas.join(",")
    )
}

fn main() {
    println!("=== o que o ponto de captura pagava com o profiler DESLIGADO ===\n");

    let m: Mutex<u64> = Mutex::new(0);
    let voltas = 200_000;
    let inicio = Instant::now();
    for _ in 0..voltas {
        *m.lock().unwrap() += 1;
    }
    let por_lock = inicio.elapsed().as_secs_f64() * 1e9 / voltas as f64;
    println!("  um lock/unlock sem disputa ......................... {por_lock:>9.1} ns");

    let mut por_parse = [0.0f64; 2];
    for (k, n) in [1usize, 5_000].into_iter().enumerate() {
        let c = corpo(n);
        let voltas = if n == 1 { 200_000 } else { 200 };
        let inicio = Instant::now();
        let mut ok = 0usize;
        for _ in 0..voltas {
            ok += Json::analisar(&c).map(|_| 1).unwrap_or(0);
        }
        assert_eq!(ok, voltas);
        por_parse[k] = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;
        println!(
            "  Json::analisar de {n:>5} linha(s), {:>7} bytes ......... {:>9.2} us",
            c.len(),
            por_parse[k]
        );
    }

    println!("\n=== o veredito ===\n");
    println!(
        "  Por pedido de UMA linha:      2 parses = {:>8.2} us  contra {:>6.2} us de lock",
        2.0 * por_parse[0],
        2.0 * por_lock / 1000.0
    );
    println!(
        "  Por lote de 5.000 linhas:     2 parses = {:>8.0} us  contra {:>6.2} us de lock",
        2.0 * por_parse[1],
        2.0 * por_lock / 1000.0
    );
    println!(
        "\n  O parse custa {:.0}x o lock no lote. Nao era o mutex: era analisar\n  \
         meio megabyte de JSON duas vezes para jogar fora.",
        por_parse[1] * 1000.0 / por_lock
    );
}
