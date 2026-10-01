//! Quanto custa absorver o diario local no mapa de toques do bidirecional --
//! pedido 330, medido ANTES do conserto.
//!
//! ```bash
//! cargo run --release --example custo-da-absorcao-do-bidi -p phxsql-store -- [eventos]
//! ```
//!
//! A receita e a do servidor (`absorver_diario_local`): lotes de 500, teto de
//! 16 MiB, `valores_da_imagem`, um `HashMap<String, _>` por chave. Tres
//! medidas, porque o pedido tem tres perguntas:
//!
//! 1. **a primeira rodada depois do arranque** -- o mapa vazio, a tabela aberta
//!    agora, o diario inteiro passando pelo mapa;
//! 2. **uma rodada qualquer depois dela**, com UM evento local novo: o servidor
//!    reabre a tabela a cada rodada, e a tabela reaberta nao tem a marca do
//!    diario -- ela caminha do comeco do volume ate `vistos`, cabecalho por
//!    cabecalho, para ler um evento;
//! 3. **a mesma rodada com a marca guardada** de uma rodada para a outra.
//!
//! O servidor faz as tres com a trava EXCLUSIVA de dados na mao: o tempo
//! medido aqui e o tempo de servidor parado, para escrita e para leitura.

use std::collections::HashMap;
use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const LOTE: u64 = 500;
const TETO: usize = 16 * 1024 * 1024;

/// O corpo da absorcao, do jeito do servidor: devolve quantos leu.
fn absorver(t: &mut Table, vistos: &mut u64, mapa: &mut HashMap<String, (i64, bool)>) -> u64 {
    let total = t.eventos().unwrap();
    let antes = *vistos;
    while *vistos < total {
        let lote = t.diario_com_imagem_ate(*vistos, LOTE, TETO).unwrap();
        if lote.is_empty() {
            break;
        }
        for (ev, imagem) in lote {
            *vistos += 1;
            if imagem.is_empty() {
                continue;
            }
            let valores = t.valores_da_imagem(&imagem).unwrap();
            let chave = match &valores[0] {
                Value::Int(i) => i.to_string(),
                outro => format!("{outro:?}"),
            };
            mapa.insert(chave, (ev.carimbo, false));
        }
    }
    *vistos - antes
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    let esquema = Schema::new(
        "t",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!("phx-absorcao-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let mut t = Table::criar(&dir, esquema).unwrap();
        t.ligar_imagem_no_diario(true);
        for i in 1..=n {
            t.inserir(&[Value::Int(i), Value::Str(format!("Cliente {i:08}"))])
                .unwrap();
        }
        t.sincronizar().unwrap();
    }

    // 1. A primeira rodada: tabela recem-aberta, mapa vazio.
    let mut mapa = HashMap::new();
    let mut vistos = 0u64;
    let inicio = Instant::now();
    let mut t = Table::abrir(&dir, "t").unwrap();
    let lidos = absorver(&mut t, &mut vistos, &mut mapa);
    let d1 = inicio.elapsed().as_secs_f64();
    let marca = t.marca_do_diario();
    drop(t);

    // 2. Uma rodada seguinte com UM evento local novo, tabela reaberta SEM marca.
    {
        let mut t = Table::abrir(&dir, "t").unwrap();
        t.ligar_imagem_no_diario(true);
        t.inserir(&[Value::Int(n + 1), Value::Str("novo".into())])
            .unwrap();
        t.sincronizar().unwrap();
    }
    let mut v2 = vistos;
    let inicio = Instant::now();
    let mut t = Table::abrir(&dir, "t").unwrap();
    let l2 = absorver(&mut t, &mut v2, &mut mapa);
    let d2 = inicio.elapsed().as_secs_f64();
    drop(t);

    // 3. A mesma rodada, COM a marca guardada da rodada anterior.
    let mut v3 = vistos;
    let inicio = Instant::now();
    let mut t = Table::abrir(&dir, "t").unwrap();
    t.definir_marca_do_diario(marca);
    let l3 = absorver(&mut t, &mut v3, &mut mapa);
    let d3 = inicio.elapsed().as_secs_f64();
    drop(t);

    println!("=== absorcao do diario local no mapa de toques, diario de {n} ===");
    println!(
        "1. primeira rodada (mapa vazio):     {lidos:>9} evento(s)  {:>9.1} ms  {:>6.2} us/evento",
        d1 * 1e3,
        d1 * 1e6 / lidos.max(1) as f64
    );
    println!(
        "2. rodada seguinte, sem marca:       {l2:>9} evento(s)  {:>9.1} ms",
        d2 * 1e3
    );
    println!(
        "3. rodada seguinte, com a marca:     {l3:>9} evento(s)  {:>9.1} ms",
        d3 * 1e3
    );
    println!("chaves no mapa: {}", mapa.len());
    let _ = std::fs::remove_dir_all(&dir);
}
