//! Onde doi a exclusao fisica: o laco de exclusoes, cronometrado sozinho.
//!
//! ```bash
//! cargo run --release --example custo-do-excluir -- <n> <m> [lote]
//! ```
//!
//! Mesmo esquema e mesmo espalhamento da bancada (`carga.rs`): insere `n`
//! linhas, sincroniza, e cronometra SO o laco de `m` exclusoes fisicas. Serve
//! para separar o custo do excluir do custo de carregar a tabela -- na bancada
//! os dois ficam no mesmo processo.
//!
//! A pergunta que ele foi escrito para responder esta em `docs/SPRINTS-CASSANDRA.md`
//! (Sprint 1): quanto do tempo e o `fsync` que `LixeiraFile::guardar` faz por
//! exclusao.
//!
//! As DUAS variantes saem daqui, e nao de uma copia editada do repositorio:
//!
//! ```bash
//! cargo run --release --example custo-do-excluir -- 200000 20000
//! PHX_EXCLUSAO_NA_JANELA=1 cargo run --release --example custo-do-excluir -- 200000 20000 200
//! PHX_EXCLUSAO_NA_JANELA=1 cargo run --release --example custo-do-excluir -- 200000 20000
//! ```
//!
//! A variavel de ambiente liga o mesmo interruptor que
//! `recursos.exclusao_na_janela` liga no servidor -- e ela existe porque
//! medicao que so se refaz editando o codigo e medicao que ninguem refaz.
//!
//! O terceiro argumento e o TAMANHO DA JANELA, e ele separa o teto da
//! entrega: `200` fecha a janela a cada 200 exclusoes, que e o que
//! `recursos.lote_operacoes` faz no servidor de verdade; sem ele a janela
//! nunca fecha, e o numero e o teto -- honesto so para a carga reservada
//! (`BULKINSERT`), em que a janela fica aberta de proposito.

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
    .expect("esquema da carga")
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n: i64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let m: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let lote: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    // O mesmo global que `recursos.exclusao_na_janela` liga no servidor.
    let na_janela =
        std::env::var("PHX_EXCLUSAO_NA_JANELA").is_ok_and(|v| v != "0" && !v.is_empty());
    phxsql_store::lixeira::definir_na_janela(na_janela);

    let dir = std::env::temp_dir().join(format!("phx-custo-do-excluir-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Table::criar(&dir, esquema())?;
    let mut t = Table::abrir(&dir, "precos")?;
    for i in 1..=n {
        t.inserir(&linha(i))?;
    }
    t.sincronizar()?;

    // O mesmo espalhamento da bancada: alvos pela tabela inteira, nao so o fim.
    let inicio = Instant::now();
    let mut feitas = 0u64;
    for k in 0..m {
        let alvo = (k * 7_919) % n.max(1) + 1;
        if t.excluir(alvo as u64)? {
            feitas += 1;
        }
        // A janela do servidor, refeita aqui: `lote_operacoes` gravacoes e o
        // `fsync` fecha a tabela inteira.
        if lote > 0 && (k + 1) % lote == 0 {
            t.sincronizar()?;
        }
    }
    let s = inicio.elapsed().as_secs_f64();
    t.sincronizar()?;

    println!(
        "excluir {feitas} de {n} ({}): {s:.3} s  ({:.2} us/linha, {:.0}/s)",
        match (na_janela, lote) {
            (false, _) => "fsync por exclusao".to_string(),
            (true, 0) => "na janela, sem fechar (o teto)".to_string(),
            (true, n) => format!("na janela, fechando a cada {n}"),
        },
        s * 1e6 / feitas.max(1) as f64,
        feitas as f64 / s.max(1e-9)
    );

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}
