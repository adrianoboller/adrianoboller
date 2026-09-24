//! Quanto custa o `fechar` que nao baixa mais o byte 52 -- pedido 522.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store      # binario velho mede o passado
//! cargo run --release --example custo-do-byte-52 -p phxsql-store -- [reps] [linhas]
//! ```
//!
//! # As duas cargas, e por que as duas
//!
//! * **lote** -- `linhas` insercoes num punho so, o `Drop` (que roda o
//!   `fechar`) e um fecho de janela numa tabela reaberta. E a carga em massa,
//!   e o `inserir` dela nao passa pelo `fechar` nenhuma vez: o que o 522 muda
//!   ali e so o fim.
//! * **servidor** -- o ciclo do `phxsqld`: a tabela e aberta e fechada a CADA
//!   pedido, e a janela fecha a cada 200. E onde o 522 mexe no laco quente: o
//!   `fechar` deixa de gravar o cabecalho com o 0 e a abertura seguinte deixa
//!   de regrava-lo com o 1, mas a abertura passa a perguntar ao atestado e o
//!   `fechar` passa a atestar.
//!
//! Mediana de `reps` corridas, com a faixa min-max ao lado: a regra do pedido
//! 155 -- vencedor so fora do ruido. O mesmo fonte compila contra o motor de
//! antes (a API usada e so a publica de sempre), e e assim que o antes e o
//! depois saem do mesmo medidor.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use std::path::Path;
use std::time::Instant;

const JANELA: i64 = 200;

fn esquema() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

const CIDADES: [&str; 4] = ["Blumenau", "Joinville", "Itajai", "Curitiba"];

fn linha(id: i64) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(format!("cliente numero {id:08}")),
        Value::Str(CIDADES[(id % 4) as usize].into()),
    ]
}

/// (us por linha no inserir, ms do Drop, ms do fecho da janela)
fn lote(dir: &Path, linhas: i64) -> (f64, f64, f64) {
    let mut t = Table::criar(dir, esquema()).unwrap();
    t.sincronizar().unwrap();
    let c = Instant::now();
    for id in 1..=linhas {
        t.inserir(&linha(id)).unwrap();
    }
    let inserir = c.elapsed().as_secs_f64() * 1e6 / linhas as f64;
    let c = Instant::now();
    drop(t);
    let fechar = c.elapsed().as_secs_f64() * 1e3;
    let c = Instant::now();
    let mut t = Table::abrir(dir, "pedidos").unwrap();
    t.sincronizar().unwrap();
    let janela = c.elapsed().as_secs_f64() * 1e3;
    assert!(!t.indice_precisa_reconstruir());
    (inserir, fechar, janela)
}

/// (us por pedido, ms por fecho de janela)
fn servidor(dir: &Path, linhas: i64) -> (f64, f64) {
    let mut t = Table::criar(dir, esquema()).unwrap();
    t.sincronizar().unwrap();
    drop(t);
    let mut em_pedidos = 0.0f64;
    let mut em_fechos = 0.0f64;
    let mut fechos = 0u32;
    for id in 1..=linhas {
        let c = Instant::now();
        {
            let mut t = Table::abrir(dir, "pedidos").unwrap();
            t.inserir(&linha(id)).unwrap();
        }
        em_pedidos += c.elapsed().as_secs_f64();
        if id % JANELA == 0 {
            let c = Instant::now();
            let mut t = Table::abrir(dir, "pedidos").unwrap();
            t.sincronizar().unwrap();
            em_fechos += c.elapsed().as_secs_f64();
            fechos += 1;
        }
    }
    let mut t = Table::abrir(dir, "pedidos").unwrap();
    assert!(!t.indice_precisa_reconstruir());
    assert_eq!(t.buscar("porId", &[Value::Int(linhas)]).unwrap().len(), 1);
    (
        em_pedidos * 1e6 / linhas as f64,
        em_fechos * 1e3 / fechos.max(1) as f64,
    )
}

fn mediana(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let reps: usize = a.first().and_then(|s| s.parse().ok()).unwrap_or(5);
    let linhas: i64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let base = std::env::temp_dir().join(format!("phx-byte52-{}", std::process::id()));

    let (mut ins, mut fec, mut jan, mut ped, mut jfe) = (vec![], vec![], vec![], vec![], vec![]);
    for r in 0..reps {
        let d = base.join(format!("lote{r}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let (i, f, j) = lote(&d, linhas);
        ins.push(i);
        fec.push(f);
        jan.push(j);
        let _ = std::fs::remove_dir_all(&d);

        let d = base.join(format!("serv{r}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let (p, j) = servidor(&d, linhas);
        ped.push(p);
        jfe.push(j);
        let _ = std::fs::remove_dir_all(&d);
    }
    let _ = std::fs::remove_dir_all(&base);

    let linha_de = |rotulo: &str, v: &mut Vec<f64>, unidade: &str| {
        let (m, lo, hi) = mediana(v);
        println!("  {rotulo:<34} {m:>9.3} {unidade}   [{lo:.3} - {hi:.3}]");
        format!("\"{rotulo}\":{{\"mediana\":{m:.4},\"min\":{lo:.4},\"max\":{hi:.4}}}")
    };
    println!("CUSTO DO BYTE 52 -- {linhas} linhas, {reps} corridas, janela de {JANELA}\n");
    let campos = [
        linha_de("lote: inserir (us/linha)", &mut ins, "us"),
        linha_de("lote: Drop/fechar (ms)", &mut fec, "ms"),
        linha_de("lote: fecho da janela (ms)", &mut jan, "ms"),
        linha_de("servidor: pedido abrir+inserir+Drop (us)", &mut ped, "us"),
        linha_de("servidor: fecho da janela (ms)", &mut jfe, "ms"),
    ];
    println!(
        "RESULTADO {{\"linhas\":{linhas},\"reps\":{reps},{}}}",
        campos.join(",")
    );
}
