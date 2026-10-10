//! Quanto o ENSAIO do grupo da replica fiel custa no laco do aplicador --
//! pedido 299, F1. O desenho dizia «raciocinado, nao medido»; este e o
//! medidor.
//!
//! ```bash
//! cargo run --release --example custo-do-ensaio -p phxsql-store -- [eventos] [pares]
//! ```
//!
//! # O que se compara
//!
//! O MESMO trabalho dos dois lados: a replica aplica os mesmos eventos, em
//! grupos de 500 (o `EVENTOS_POR_TOMADA` do `Juntador`), num diretorio novo a
//! cada corrida. O lado A ensaia o grupo inteiro antes de aplica-lo
//! (`Table::ensaiar_evento`, depois `ver_so_o_disco`); o lado B so aplica. A
//! mistura e a do desenho: 90% inclusao, 10% alteracao de uma linha
//! anterior. Duas variantes do ensaio: sem a unicidade (a replica
//! `somente_leitura`, o caso comum) e com ela (a que pode ter sido escrita
//! localmente, que decodifica cada imagem).
//!
//! Os pares se intercalam (AB, BA, AB...) para o aquecimento do disco e do
//! alocador nao cair sempre no mesmo lado, e cada lado sai com a mediana e a
//! faixa min-max: so ha diferenca se as faixas nao se cruzam.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
use phxsql_store::table::{EnsaioDaTabela, Table};

/// O tamanho do grupo: o `EVENTOS_POR_TOMADA` do `Juntador` (servidor).
const GRUPO: usize = 500;

fn esquema() -> Schema {
    Schema::new(
        "itens",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("venda", ColumnType::Int8),
            Column::new("descricao", ColumnType::Str(40)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
            IndexDef::new("porVenda", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64, rev: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Int(i / 8),
        Value::Str(format!("item {i:08} rev {rev}")),
    ]
}

fn dir_limpo(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("r99-ensaio-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// O lado de uma corrida: sem ensaio, ensaio sem unicidade, ensaio com.
#[derive(Clone, Copy, PartialEq)]
enum Lado {
    SemEnsaio,
    Ensaio,
    EnsaioComUnicidade,
}

/// Aplica tudo numa replica nova e devolve us/evento.
fn correr(eventos: &[(Operacao, u64, Vec<u8>)], lado: Lado, rotulo: &str) -> f64 {
    let dir = dir_limpo(rotulo);
    let mut r = Table::criar(&dir, esquema()).unwrap();
    let t0 = Instant::now();
    for grupo in eventos.chunks(GRUPO) {
        if lado != Lado::SemEnsaio {
            let mut ensaio = EnsaioDaTabela::novo(lado == Lado::EnsaioComUnicidade);
            for (op, rowid, imagem) in grupo {
                r.ensaiar_evento(*op, *rowid, imagem, &mut ensaio).unwrap();
            }
            r.ver_so_o_disco();
        }
        for (op, rowid, imagem) in grupo {
            r.aplicar_evento(*op, *rowid, imagem).unwrap();
        }
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / eventos.len() as f64;
    drop(r);
    let _ = std::fs::remove_dir_all(&dir);
    us
}

fn resumo(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);
    let pares: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    // A origem: 90% inclusao, 10% alteracao de uma linha anterior.
    let dir_s = dir_limpo("origem");
    let mut s = Table::criar(&dir_s, esquema())
        .unwrap()
        .com_imagem_no_diario(true);
    let mut incluidas = 0i64;
    for k in 0..n {
        if k % 10 == 9 && incluidas > 0 {
            let alvo = 1 + (k as i64 * 7919) % incluidas;
            s.atualizar(alvo as u64, &linha(alvo, k as i64)).unwrap();
        } else {
            incluidas += 1;
            s.inserir(&linha(incluidas, 0)).unwrap();
        }
    }
    let eventos: Vec<(Operacao, u64, Vec<u8>)> = s
        .diario_com_imagem(0, 0)
        .unwrap()
        .into_iter()
        .map(|(e, imagem)| (e.operacao, e.rowid, imagem))
        .collect();
    drop(s);
    let _ = std::fs::remove_dir_all(&dir_s);
    let alteracoes = eventos
        .iter()
        .filter(|(op, _, _)| *op == Operacao::Alteracao)
        .count();
    println!(
        "{} eventos ({} alteracoes), grupos de {GRUPO}, {pares} pares intercalados por variante",
        eventos.len(),
        alteracoes
    );

    for (nome, lado) in [
        ("ensaio sem unicidade", Lado::Ensaio),
        ("ensaio com unicidade", Lado::EnsaioComUnicidade),
    ] {
        let mut com = Vec::new();
        let mut sem = Vec::new();
        for p in 0..pares {
            if p % 2 == 0 {
                com.push(correr(&eventos, lado, "a"));
                sem.push(correr(&eventos, Lado::SemEnsaio, "b"));
            } else {
                sem.push(correr(&eventos, Lado::SemEnsaio, "b"));
                com.push(correr(&eventos, lado, "a"));
            }
        }
        let (mc, minc, maxc) = resumo(&mut com);
        let (ms, mins, maxs) = resumo(&mut sem);
        let cruzam = minc <= maxs && mins <= maxc;
        println!(
            "{nome}: com {mc:.2} us/evento [{minc:.2}-{maxc:.2}] | sem {ms:.2} \
             [{mins:.2}-{maxs:.2}] | mediana {:+.1}% | faixas {}",
            (mc / ms - 1.0) * 100.0,
            if cruzam {
                "SE CRUZAM (dentro do ruido)"
            } else {
                "separadas"
            }
        );
    }
}
