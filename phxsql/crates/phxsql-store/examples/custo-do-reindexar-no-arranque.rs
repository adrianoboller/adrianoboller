//! Quanto custa reconstruir o indice de uma tabela no ARRANQUE.
//!
//! # A premissa que este medidor existe para conferir
//!
//! O pedido 172 pergunta se a recuperacao devia reconstruir o `.ndx` da filha
//! em vez de recusar cascatear para ela. A pergunta nao e «da para consertar»
//! -- a maquina ja existe (`Table::reindexar`) --, e sim **quanto custa
//! reconstruir no arranque**, porque trocar o conservador pelo automatico sem
//! numero e o que o pedido 113 ja cobrou uma vez.
//!
//! E ha um segundo numero, que muda a leitura do primeiro: a recuperacao JA
//! reconstroi -- `transacao.rs:1176` chama `reindexar()` para toda tabela
//! NOMEADA NA MARCA. O que ela nao alcanca e a filha da cascata, porque a
//! cascata nao entra na marca. Entao o custo medido aqui nao e um custo novo:
//! e o mesmo que a recuperacao ja paga hoje pelas outras tabelas.
//!
//! Roda com:
//! `cargo run --release --example custo-do-reindexar-no-arranque -p phxsql-store`

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use std::time::Instant;

/// Os tamanhos medidos. O ultimo e maior que qualquer tabela desta bancada de
/// proposito: o numero que decide «cabe no arranque?» e o do pior caso.
const TAMANHOS: [u64; 4] = [1_000, 10_000, 100_000, 500_000];

fn dir(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-reidx-{rotulo}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A forma de uma filha de verdade: chave propria unica mais o indice da
/// coluna da chave estrangeira, que e o que a busca reversa exige.
fn filha(d: &std::path::Path, n: u64) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
            Column::new("descricao", ColumnType::Str(60)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    let mut t = Table::criar(d, e).unwrap();
    let mut lote = Vec::with_capacity(n as usize);
    for i in 1..=n {
        lote.push(vec![
            Value::Int(i as i64),
            // Mil maes: a filha aponta para uma faixa realista, e nao para
            // uma chave so, que deixaria a arvore degenerada.
            Value::Int((i % 1_000) as i64),
            Value::Str(format!("linha {i}")),
        ]);
    }
    t.inserir_lote(&lote, true).unwrap();
    t.sincronizar().unwrap();
    t
}

fn main() {
    println!("Reconstruir o .ndx no arranque -- custo por tamanho da tabela\n");
    println!(
        "{:>10}  {:>12}  {:>12}  {:>10}",
        "linhas", "reindexar", "por linha", "chaves"
    );
    println!("{}", "-".repeat(50));

    for n in TAMANHOS {
        let d = dir(&format!("n{n}"));
        let mut t = filha(&d, n);

        // Tres voltas, mediana -- uma volta so mede o cache do sistema de
        // arquivos tanto quanto o nosso codigo.
        let mut tempos = Vec::new();
        let mut chaves = 0u64;
        for _ in 0..3 {
            let t0 = Instant::now();
            let saida = t.reindexar().unwrap();
            tempos.push(t0.elapsed());
            chaves = saida.iter().map(|(_, q)| q).sum();
        }
        tempos.sort();
        let mediana = tempos[1];

        println!(
            "{:>10}  {:>10.1} ms  {:>9.2} us  {:>10}",
            n,
            mediana.as_secs_f64() * 1_000.0,
            mediana.as_secs_f64() * 1_000_000.0 / n as f64,
            chaves
        );
        std::fs::remove_dir_all(&d).ok();
    }

    println!(
        "\nA leitura: o numero que decide nao e o total, e o total NO PIOR CASO\n\
         de uma queda -- uma filha por vez, e so as filhas que a cascata tocou."
    );
}
