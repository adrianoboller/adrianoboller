//! O `.fts` de verdade contra a insercao de hoje -- e o erro de forma que o
//! medidor anterior cometeu.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! cargo run --release --example custo-do-fts-de-verdade -- [linhas]
//! ```
//!
//! # O erro que este medidor existe para consertar
//!
//! O `custo-da-chave-a-mais` mediu **15 INDICES SEPARADOS** e achou um
//! penhasco: 5,46 us por insercao com 1 indice, 49,37 us com 15 -- **9,05x**.
//! Dali saiu a decisao de desenho do `FTS.md` §4.3: o `.fts` nao entra
//! sincrono, entra por despejo em lote.
//!
//! **A decisao saiu de uma medida da forma errada.** Quinze indices sao
//! quinze arvores B+, cada uma com o proprio conjunto de paginas quentes. O
//! `.fts` e **UMA arvore** recebendo ~14 chaves por linha. O conjunto quente
//! de uma arvore nao e o de quinze, e a causa que eu mesmo nomeei para o
//! penhasco -- «as paginas de 15 arvores deixam de caber no cache» -- diz
//! justamente que a diferenca de forma e o que decide.
//!
//! *Bancada compara trabalho igual, e nao so pergunta igual.* Eu comparei
//! pergunta igual (14 chaves a mais) com trabalho diferente (14 arvores a
//! mais). Este medidor compara o trabalho que o `.fts` faz de verdade.
//!
//! # As tres medidas
//!
//! | medida | o que entra |
//! |--------|-------------|
//! | A `so a tabela` | `inserir` numa tabela com 1 indice. O chao de hoje. |
//! | B `+ fts junto` | A, e a cada linha as ~14 chaves entram no `.fts` na hora. |
//! | C `+ fts em lote` | A, com as chaves guardadas e despejadas a cada `lote`. |
//!
//! `B / A` responde se a escrita sincrona cabe. `C / B` responde se o lote
//! compra alguma coisa -- e essa segunda pergunta tambem estava por medir: o
//! `FTS.md` prescreveu o lote como cura de uma causa que ele proprio marcou
//! como NAO MEDIDA.
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::fts::{FtsFile, EXT_FTS};
use phxsql_store::table::Table;

const RECHEIO: [&str; 12] = [
    "pedido",
    "cliente",
    "nota",
    "fiscal",
    "entrega",
    "produto",
    "valor",
    "desconto",
    "parcela",
    "vencimento",
    "transportadora",
    "observacao",
];

fn esquema() -> Schema {
    Schema::new(
        "docs",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("corpo", ColumnType::Str(200)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
}

/// ~14 palavras distintas, como o texto que a §20 mediu.
fn texto(i: u64) -> String {
    let mut s = String::with_capacity(220);
    for k in 0..14 {
        s.push_str(RECHEIO[((i + k) as usize) % RECHEIO.len()]);
        s.push('_');
        // um sufixo por linha faz os termos serem MUITOS, que e o caso real de
        // um indice de texto: um vocabulario pequeno caberia todo numa pagina.
        s.push_str(&((i + k) % 5_000).to_string());
        s.push(' ');
    }
    s.truncate(200);
    s
}

fn preparar(dir: &str) -> Table {
    let _ = std::fs::remove_dir_all(dir);
    Table::criar(dir, esquema()).expect("criar")
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let linhas: u64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(50_000);
    let lote: u64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(200);
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());

    println!("O `.fts` de verdade: UMA arvore com ~14 chaves por linha");
    println!("{linhas} linhas, lote de {lote} (o `lote_operacoes` padrao do servidor)");
    println!();

    // A -- so a tabela.
    let dir_a = format!("{base}/phx-ftsv-a");
    let mut ta = preparar(&dir_a);
    let inicio = Instant::now();
    for i in 0..linhas {
        ta.inserir(&[Value::Int(i as i64), Value::Str(texto(i))])
            .unwrap();
    }
    let s_a = inicio.elapsed().as_secs_f64();
    drop(ta);

    // B -- a tabela e o `.fts` juntos, linha a linha.
    let dir_b = format!("{base}/phx-ftsv-b");
    let mut tb = preparar(&dir_b);
    let mut fb = FtsFile::criar(format!("{dir_b}/docs.{EXT_FTS}"), vec![true]).unwrap();
    let inicio = Instant::now();
    let mut chaves_b = 0usize;
    for i in 0..linhas {
        let t = texto(i);
        let r = tb
            .inserir(&[Value::Int(i as i64), Value::Str(t.clone())])
            .unwrap();
        chaves_b += fb.indexar(0, r, &t).unwrap();
    }
    let s_b = inicio.elapsed().as_secs_f64();
    drop(tb);

    // C -- o mesmo, com as chaves guardadas e despejadas a cada `lote`.
    let dir_c = format!("{base}/phx-ftsv-c");
    let mut tc = preparar(&dir_c);
    let mut fc = FtsFile::criar(format!("{dir_c}/docs.{EXT_FTS}"), vec![true]).unwrap();
    let inicio = Instant::now();
    let mut pendentes: Vec<(u64, String)> = Vec::with_capacity(lote as usize);
    for i in 0..linhas {
        let t = texto(i);
        let r = tc
            .inserir(&[Value::Int(i as i64), Value::Str(t.clone())])
            .unwrap();
        pendentes.push((r, t));
        if pendentes.len() as u64 >= lote {
            for (r, t) in pendentes.drain(..) {
                fc.indexar(0, r, &t).unwrap();
            }
        }
    }
    for (r, t) in pendentes.drain(..) {
        fc.indexar(0, r, &t).unwrap();
    }
    let s_c = inicio.elapsed().as_secs_f64();
    drop(tc);

    let us = |s: f64| s * 1_000_000.0 / linhas as f64;
    println!(
        "{:>18}  {:>12}  {:>12}  {:>8}",
        "medida", "total ms", "us/linha", "x sobre A"
    );
    println!("{}", "-".repeat(60));
    for (rot, s) in [
        ("A so a tabela", s_a),
        ("B + fts junto", s_b),
        ("C + fts em lote", s_c),
    ] {
        println!(
            "{rot:>18}  {:>12.1}  {:>12.3}  {:>8.2}",
            s * 1000.0,
            us(s),
            s / s_a
        );
    }

    println!();
    println!("As duas perguntas que estavam por medir:");
    println!(
        "  1. a escrita SINCRONA cabe?      B/A = {:.2}x   ({:.1} us por chave)",
        s_b / s_a,
        (s_b - s_a) * 1_000_000.0 / chaves_b as f64
    );
    println!("  2. o LOTE compra alguma coisa?   C/B = {:.2}x", s_c / s_b);
    println!();
    println!("  para comparar: o custo-da-chave-a-mais mediu 9,05x com 15 ARVORES");
    println!("  separadas, que e uma forma diferente da que o `.fts` tem.");

    println!();
    println!(
        "RESULTADO {{\"linhas\":{linhas},\"lote\":{lote},\"chaves\":{chaves_b},\
         \"a_ms\":{:.1},\"b_ms\":{:.1},\"c_ms\":{:.1},\
         \"b_sobre_a\":{:.2},\"c_sobre_b\":{:.2}}}",
        s_a * 1000.0,
        s_b * 1000.0,
        s_c * 1000.0,
        s_b / s_a,
        s_c / s_b
    );

    for d in [&dir_a, &dir_b, &dir_c] {
        let _ = std::fs::remove_dir_all(d);
    }
}
