//! O `sincronizar` a cada 200 operacoes custa quanto por linha, de verdade?
//!
//! ```bash
//! cargo run --release --example custo-do-fsync -- [tamanhos separados por espaco]
//! ```
//!
//! A leitura do Cassandra levantou a suspeita: la o cliente NUNCA executa
//! `fsync` -- nem no modo `batch`, quem sincroniza e uma thread propria
//! (`AbstractCommitLogService.java:154`). Aqui a janela de durabilidade
//! (`Durabilidade::PorLote`) fecha a cada 200 operacoes DENTRO da operacao que
//! deu o azar de ser a 200a.
//!
//! A proposta de tirar o `fsync` do caminho da operacao so vale se ele custar
//! algo que se veja. A regra da casa manda medir a premissa antes do item --
//! ja derrubou seis diagnosticos plausiveis -- entao este medidor existe para
//! decidir a vida do item: **se a diferenca por linha ficar abaixo de 2% do
//! custo da bancada (0,46 us), o item morre e este arquivo e a entrega.**
//!
//! Ele usa o MESMO esquema da bancada (5 colunas, com Decimal e Date), porque
//! o 4.8 do DESEMPENHO.md ja mostrou que esquema diferente mede outra coisa.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

fn esquema() -> Schema {
    Schema::new(
        "precos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
            Column::new(
                "valor",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("cadastro", ColumnType::Date),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
        Value::Decimal(((i % 900_000) + 100) as i128),
        Value::Date(20_000 + (i % 400) as i32),
    ]
}

/// Insere `n` linhas a partir de `de`, sincronizando a cada `cada`
/// (zero = uma vez so, no fim). Devolve segundos.
fn trecho(t: &mut Table, de: i64, n: i64, cada: i64) -> f64 {
    let inicio = Instant::now();
    for i in de + 1..=de + n {
        t.inserir(&linha(i)).unwrap();
        if cada > 0 && (i - de) % cada == 0 {
            t.sincronizar().unwrap();
        }
    }
    t.sincronizar().unwrap();
    inicio.elapsed().as_secs_f64()
}

fn main() {
    let tamanhos: Vec<i64> = {
        let v: Vec<i64> = std::env::args()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();
        if v.is_empty() {
            vec![1_000_000, 3_000_000]
        } else {
            v
        }
    };
    const AMOSTRA: i64 = 20_000;
    const JANELA: i64 = 200; // o `a_cada` padrao da Durabilidade::PorLote

    println!("=== sincronizar a cada {JANELA} contra uma vez so, {AMOSTRA} linhas por trecho ===\n");
    println!(
        "  {:>10}  {:>12} {:>12} {:>10}  {:>14}",
        "ja tinha", "a cada 200", "uma vez so", "delta", "sync avulso"
    );

    for &n in &tamanhos {
        let dir = std::env::temp_dir().join(format!("phx-fsync-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = Table::criar(&dir, esquema()).unwrap();
        for i in 1..=n {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();

        // Trechos INTERCALADOS (aba, uma, aba, uma), para o ruido da maquina
        // cair dos dois lados em vez de num so.
        let mut com = 0.0;
        let mut sem = 0.0;
        let mut feito = n;
        for _ in 0..2 {
            com += trecho(&mut t, feito, AMOSTRA, JANELA);
            feito += AMOSTRA;
            sem += trecho(&mut t, feito, AMOSTRA, 0);
            feito += AMOSTRA;
        }
        let com_us = com * 1e6 / (2.0 * AMOSTRA as f64);
        let sem_us = sem * 1e6 / (2.0 * AMOSTRA as f64);

        // E um `sincronizar` sozinho, logo depois de 200 insercoes: o que a
        // 200a operacao paga a mais que as 199 vizinhas.
        let _ = trecho(&mut t, feito, JANELA, 0);
        let inicio = Instant::now();
        t.sincronizar().unwrap();
        let avulso_ms = inicio.elapsed().as_secs_f64() * 1e3;

        println!(
            "  {n:>10}  {com_us:>9.2} us {sem_us:>9.2} us {:>7.2} us  {avulso_ms:>11.3} ms",
            com_us - sem_us
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    println!(
        "\n  «delta» e o que tirar o sincronizar do caminho da operacao\n  \
         compraria, por linha. O criterio combinado antes de medir: abaixo\n  \
         de 0,46 us/linha (2% dos 23,0 us da bancada), o item morre aqui."
    );
}
