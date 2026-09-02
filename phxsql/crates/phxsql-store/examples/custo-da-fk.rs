//! Quanto custa conferir a chave estrangeira -- e quanto custa quando ninguem
//! pediu.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store   # binario velho mede o passado
//! cargo run --release --example custo-da-fk -- [linhas]
//! ```
//!
//! Tres perguntas, nesta ordem:
//!
//! 1. **A tabela sem chave conferida paga alguma coisa?** E o portao do
//!    custo-zero -- a licao do Profiler, que cobrava 7% da carga fazendo
//!    trabalho ANTES de perguntar se estava ligado. A resposta tem de ser
//!    "nao mensuravel", e o controle e a MESMA tabela com a chave declarada e
//!    `verificar` desligado: a diferenca entre as duas e so o interruptor.
//! 2. **Quanto custa com a conferencia ligada?** Este e o numero que decide se
//!    a garantia cabe numa carga, e ele nao se estima.
//! 3. **De onde vem o custo?** Abrir a mae a cada linha e a suspeita obvia, e
//!    suspeita obvia e onde esta casa ja errou. O medidor separa a abertura da
//!    busca no indice, para o diagnostico ser medido e nao plausivel.
//!
//! Nenhum numero aqui vem de outro dia: todo numero saiu da rodada que o
//! imprimiu.

use std::time::Instant;

use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn maes(dir: &std::path::Path, quantas: i64) -> Table {
    let esquema = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema da mae");
    let mut t = Table::criar(dir, esquema).unwrap();
    for i in 1..=quantas {
        t.inserir(&[Value::Int(i)]).unwrap();
    }
    t
}

fn filhas(dir: &std::path::Path, nome: &str, conferindo: bool) -> Table {
    let esquema = Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema da filha");
    let esquema = esquema
        .com_chaves_estrangeiras(vec![ForeignKey::new(
            "fk_cliente",
            vec![1],
            "clientes",
            vec!["id".into()],
        )
        .conferindo(conferindo)])
        .expect("chave da filha");
    Table::criar(dir, esquema).unwrap()
}

fn cronometrar(t: &mut Table, de: i64, linhas: i64, maes: i64) -> f64 {
    let inicio = Instant::now();
    for i in de..(de + linhas) {
        // Espalha pelas maes para nao medir sempre a mesma pagina do indice.
        t.inserir(&[Value::Int(i), Value::Int(1 + (i % maes))])
            .unwrap();
    }
    inicio.elapsed().as_secs_f64() * 1e6 / linhas as f64
}

fn main() {
    let linhas: i64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(20_000);
    const MAES: i64 = 1_000;

    let base = std::env::temp_dir().join(format!("phx-custo-fk-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();

    let mut mae = maes(&base, MAES);
    mae.sincronizar().unwrap();
    drop(mae);
    let mut sem = filhas(&base, "sem_conferir", false);
    let mut com = filhas(&base, "com_conferir", true);

    // Aquece: a primeira insercao paga abertura de arquivo que nao e do teste.
    cronometrar(&mut sem, 1, 200, MAES);
    cronometrar(&mut com, 1, 200, MAES);

    let us_sem = cronometrar(&mut sem, 1_000, linhas, MAES);
    let us_com = cronometrar(&mut com, 1_000, linhas, MAES);

    // De onde vem o custo: abrir a mae, sozinho, sem a busca.
    let inicio = Instant::now();
    for _ in 0..linhas {
        let _ = Table::abrir(&base, "clientes").unwrap();
    }
    let us_abrir = inicio.elapsed().as_secs_f64() * 1e6 / linhas as f64;

    println!("linhas: {linhas}, maes: {MAES}");
    println!("  chave declarada, NAO conferida  {us_sem:8.2} us/linha");
    println!("  chave CONFERIDA                 {us_com:8.2} us/linha");
    println!(
        "  diferenca                       {:8.2} us/linha  ({:.2}x)",
        us_com - us_sem,
        us_com / us_sem
    );
    println!("  --- de onde vem:");
    println!("  so ABRIR a mae                  {us_abrir:8.2} us");
    println!(
        "  a busca no indice (o resto)     {:8.2} us",
        (us_com - us_sem) - us_abrir
    );
    let fatia = 100.0 * us_abrir / (us_com - us_sem);
    println!("  a abertura e {fatia:.1}% do custo da conferencia");

    std::fs::remove_dir_all(&base).ok();
}
