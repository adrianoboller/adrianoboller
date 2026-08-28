//! Carga de trabalho para a comparacao com outros motores.
//!
//! ```bash
//! cargo run --release --example carga -- <dir> <fase> <n>
//! ```
//!
//! Cada fase roda num PROCESSO SEPARADO de proposito: assim o medidor externo
//! le `/proc/<pid>/io` e `/proc/<pid>/status` do processo que fez exatamente
//! aquela fase, sem misturar com as outras. E o mesmo tratamento que o mysqld
//! recebe do outro lado -- la os contadores tambem sao lidos por diferenca.
//!
//! Fases: `criar`, `inserir`, `buscar`, `varrer`, `atualizar`, `excluir`.
//!
//! A ultima linha da saida e sempre `RESULTADO <json>`, para o medidor nao
//! precisar adivinhar nada.

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

/// Gerador previsivel: as duas bancadas recebem exatamente os mesmos dados.
/// Sem sorteio -- comparar motores com entradas diferentes nao compara nada.
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
    if args.len() < 2 {
        eprintln!("uso: carga <dir> <fase> [n]");
        std::process::exit(2);
    }
    let dir = std::path::PathBuf::from(&args[0]);
    let fase = args[1].as_str();
    let n: i64 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    let inicio = Instant::now();
    let mut feitas = 0u64;

    match fase {
        "criar" => {
            std::fs::create_dir_all(&dir)?;
            Table::criar(&dir, esquema())?;
        }
        "inserir" => {
            let mut t = Table::abrir(&dir, "precos")?;
            let ja = t.slots() as i64;
            for i in (ja + 1)..=(ja + n) {
                t.inserir(&linha(i))?;
                feitas += 1;
            }
            // Um unico sincronizar no fim: e assim que se carrega em massa,
            // dos dois lados. O mysqld recebe o mesmo tratamento (uma
            // transacao por lote, nao por linha).
            t.sincronizar()?;
        }
        "buscar" => {
            // Busca pontual pelo indice unico, como um SELECT ... WHERE id = ?.
            let mut t = Table::abrir(&dir, "precos")?;
            let total = t.slots() as i64;
            let mut achados = 0u64;
            for k in 0..n {
                // Espalhado pela tabela inteira, para nao medir so o cache.
                let alvo = (k * 7_919) % total.max(1) + 1;
                let rowids = t.buscar("porId", &[Value::Int(alvo)])?;
                if let Some(r) = rowids.first() {
                    if t.ler(*r)?.is_some() {
                        achados += 1;
                    }
                }
                feitas += 1;
            }
            println!("achados: {achados}");
        }
        "varrer" => {
            // Faixa por indice nao-unico: TODAS as linhas de uma cidade, e a
            // soma do valor. Precisa ser todas: do outro lado o motor recebe
            // um COUNT(*) + SUM(valor), que le a faixa inteira. Ler so um
            // pedaco daria vantagem pelo tamanho do trabalho, nao pela
            // velocidade -- foi exatamente esse o defeito da primeira bancada.
            let mut t = Table::abrir(&dir, "precos")?;
            let rowids = t.buscar("porCidade", &[Value::Str("Blumenau".into())])?;
            let mut somados = 0u64;
            let mut soma: i128 = 0;
            for r in rowids.iter() {
                if let Some(linha) = t.ler(*r)? {
                    if let Some(Value::Decimal(v)) = linha.get(3) {
                        soma += *v;
                    }
                    somados += 1;
                }
            }
            feitas = somados;
            println!("linhas da cidade: {} soma: {soma}", rowids.len());
        }
        "atualizar" => {
            let mut t = Table::abrir(&dir, "precos")?;
            let total = t.slots() as i64;
            for k in 0..n {
                let alvo = (k * 7_919) % total.max(1) + 1;
                let mut l = linha(alvo);
                l[3] = Value::Decimal(999_900);
                t.atualizar(alvo as u64, &l)?;
                feitas += 1;
            }
            t.sincronizar()?;
        }
        "excluir" => {
            let mut t = Table::abrir(&dir, "precos")?;
            let total = t.slots() as i64;
            for k in 0..n {
                let alvo = (k * 7_919) % total.max(1) + 1;
                let _ = t.excluir(alvo as u64)?;
                feitas += 1;
            }
            t.sincronizar()?;
        }
        outra => {
            eprintln!("fase desconhecida: {outra}");
            std::process::exit(2);
        }
    }

    let s = inicio.elapsed().as_secs_f64();
    println!(
        "RESULTADO {{\"fase\":\"{fase}\",\"operacoes\":{feitas},\"segundos\":{s:.4},\
         \"por_segundo\":{:.1}}}",
        if s > 0.0 { feitas as f64 / s } else { 0.0 }
    );
    Ok(())
}
