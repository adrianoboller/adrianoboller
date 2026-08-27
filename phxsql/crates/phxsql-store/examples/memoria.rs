//! Mede o que a tabela em memoria custa e o que ela paga.
//!
//! ```bash
//! cargo run --release --example memoria
//! ```
//!
//! Monta uma tabela, faz a mesma pergunta pelos dois caminhos -- varrendo o
//! `.reg` e consultando a copia em RAM -- e imprime os dois tempos. Os numeros
//! do dossie saem daqui, e por isso este exemplo existe: numero de desempenho
//! que ninguem consegue refazer e numero em que nao se deve acreditar.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::memoria::{Consulta, Filtro, Operador, Ordem, TabelaMemoria};
use phxsql_store::table::Table;

const LINHAS: i64 = 50_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("phxsql-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;

    let esquema = Schema::new(
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
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )?;

    let cidades = [
        "Blumenau",
        "Joinville",
        "Itajai",
        "Curitiba",
        "Chapeco",
        "Lages",
    ];
    let mut t = Table::criar(&dir, esquema)?;

    let inicio = Instant::now();
    for i in 1..=LINHAS {
        t.inserir(&[
            Value::Int(i),
            Value::Str(format!("Produto {i:06}")),
            Value::Str(cidades[(i as usize) % cidades.len()].into()),
            Value::Decimal(((i % 9_000) + 100) as i128 * 100),
        ])?;
    }
    t.sincronizar()?;
    let gravou = inicio.elapsed();
    println!(
        "gravou {LINHAS} linhas em {:.2}s  ({:.0} linhas/s, um unico sincronizar no fim)",
        gravou.as_secs_f64(),
        LINHAS as f64 / gravou.as_secs_f64()
    );

    // ------------------------------------------------------------- carga
    let inicio = Instant::now();
    let m = TabelaMemoria::carregar(&mut t, &[2], 0)?;
    let carga = inicio.elapsed();
    println!(
        "carregou para a RAM em {:.0} ms  ({} linhas, {} KB de valores, mapa em \"cidade\")",
        carga.as_secs_f64() * 1000.0,
        m.vivos(),
        m.bytes() / 1024
    );

    // ---------------------------------------- a mesma pergunta, dois caminhos
    // "as linhas de Blumenau, da mais cara para a mais barata, as 10 primeiras"

    let inicio = Instant::now();
    let todas = t.varrer()?;
    let mut do_disco: Vec<_> = todas
        .into_iter()
        .filter(|(_, l)| l[2] == Value::Str("Blumenau".into()))
        .collect();
    do_disco.sort_by(|a, b| match (&a.1[3], &b.1[3]) {
        (Value::Decimal(x), Value::Decimal(y)) => y.cmp(x),
        _ => std::cmp::Ordering::Equal,
    });
    do_disco.truncate(10);
    let disco = inicio.elapsed();

    let consulta = Consulta {
        onde: vec![Filtro {
            coluna: 2,
            op: Operador::Igual,
            valor: Value::Str("Blumenau".into()),
        }],
        ordenar: vec![Ordem {
            coluna: 3,
            desc: true,
        }],
        max: 10,
        ..Default::default()
    };
    // Uma volta a seco: a primeira consulta paga o aquecimento do cache do
    // processador, e medir isso seria medir a maquina, nao o motor.
    let _ = m.selecionar(&consulta)?;
    let inicio = Instant::now();
    let da_memoria = m.selecionar(&consulta)?;
    let ram = inicio.elapsed();

    println!();
    println!("mesma pergunta: as 10 maiores de Blumenau");
    println!(
        "  varrendo o .reg    {:>9.0} us   ({} linhas lidas do disco)",
        disco.as_secs_f64() * 1e6,
        LINHAS
    );
    println!(
        "  SelectMemory       {:>9.0} us   ({} linhas examinadas, mapa em \"{}\")",
        ram.as_secs_f64() * 1e6,
        da_memoria.examinadas,
        da_memoria.por_mapa.as_deref().unwrap_or("nenhum")
    );
    println!(
        "  diferenca          {:>9.0}x",
        disco.as_secs_f64() / ram.as_secs_f64().max(1e-9)
    );

    // As duas respostas TEM de ser a mesma. Um benchmark que compara duas
    // respostas diferentes nao mede nada.
    assert_eq!(do_disco.len(), da_memoria.linhas.len());
    for (a, b) in do_disco.iter().zip(da_memoria.linhas.iter()) {
        assert_eq!(a.0, b.0, "as duas respostas divergiram");
    }
    println!();
    println!("as duas respostas conferem, linha por linha.");

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
