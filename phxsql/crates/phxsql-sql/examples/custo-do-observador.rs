//! Quanto o observador de injecao custa no pedido `sql` (pedido 495, F3)?
//!
//!     cargo run --release --example custo-do-observador -p phxsql-sql
//!
//! Mede, por texto, o menor de 9 voltas de 20.000 (o menor e o que o ruido
//! da maquina nao empurra para cima):
//!
//! * `parse` -- o `analisar_comando` de sempre (lexico + sintaxe);
//! * `observado` -- o caminho da F3: UMA leitura com os comentarios, as
//!   quatro classes sobre ela, e a sintaxe sobre a MESMA lista
//!   (`analisar_comando_dos_simbolos`);
//! * `relendo` -- o caminho que a F3 recusa: o parse de sempre e mais uma
//!   leitura so para o observador.
//!
//! # O que este medidor desmentiu
//!
//! O desenho escreveu o RED da F3 como «relexar no gancho passa de 2x o
//! parse». Nao passa: a sintaxe pesa mais que o lexico, e reler custa 1,1x a
//! 1,6x o parse. Por isso a prova no servidor CONTA passadas do lexico
//! (`o_observador_nao_acrescenta_passada_do_lexico`), em vez de cronometrar
//! contra uma regua que nunca reprovaria.

use std::hint::black_box;
use std::time::Instant;

fn menor_de_9(mut f: impl FnMut()) -> f64 {
    // Uma volta jogada fora: a primeira medida da primeira linha saia com o
    // cache frio e o parse parecia mais caro que o parse mais uma leitura.
    for _ in 0..20_000 {
        f();
    }
    let mut voltas = Vec::with_capacity(9);
    for _ in 0..9 {
        let t = Instant::now();
        for _ in 0..20_000 {
            f();
        }
        voltas.push(t.elapsed().as_nanos() as f64 / 20_000.0);
    }
    voltas.into_iter().fold(f64::INFINITY, f64::min)
}

fn main() {
    println!("ns por comando, o menor de 9 voltas de 20.000\n");
    println!(
        "{:>8} {:>10} {:>9} {:>9} {:>9}  texto",
        "parse", "observado", "x parse", "relendo", "x parse"
    );
    for sql in [
        "SELECT * FROM clientes WHERE nome = '' OR '1'='1'",
        "SELECT id, nome FROM clientes WHERE cidade = 'Blumenau' AND idade > 30 ORDER BY nome LIMIT 10",
        "INSERT INTO clientes (id, nome, cidade) VALUES (1, 'Ana', 'Blumenau')",
        "UPDATE clientes SET cidade = 'Joinville' WHERE id = 7",
    ] {
        let parse = menor_de_9(|| {
            black_box(phxsql_sql::analisar_comando(black_box(sql)).unwrap());
        });
        let observado = menor_de_9(|| {
            let s = phxsql_sql::lexico::analisar_com_comentarios(black_box(sql)).unwrap();
            black_box(phxsql_sql::sinais(&s));
            black_box(phxsql_sql::analisar_comando_dos_simbolos(s, sql, &[]).unwrap());
        });
        let relendo = menor_de_9(|| {
            black_box(phxsql_sql::analisar_comando(black_box(sql)).unwrap());
            let s = phxsql_sql::lexico::analisar_com_comentarios(black_box(sql)).unwrap();
            black_box(phxsql_sql::sinais(&s));
        });
        println!(
            "{parse:>8.0} {observado:>10.0} {:>9.2} {relendo:>9.0} {:>9.2}  {sql}",
            observado / parse,
            relendo / parse
        );
    }
}
