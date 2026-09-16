//! Quais arquivos cada operacao de ponto manda ao disco em `por_operacao`,
//! e quantos deles estavam LIMPOS -- medido pelo nucleo, nao lido no fonte.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store          # binario velho mede o passado
//! cargo run --release --example fsync-por-operacao -p phxsql-store
//! cargo run --release --example fsync-por-operacao -p phxsql-store -- --tempo
//! cargo run --release --example fsync-por-operacao -p phxsql-store -- --tempo 5 2000
//! ```
//!
//! # A pergunta, e por que ela se mede assim
//!
//! A bancada CRUD (`bancada/colmeia/medir-crud.py`, pedido 257) contou por
//! `strace -c` que o padrao paga **8 `fsync`** por inserir e **9** por excluir
//! no regime `por_operacao`, e apontou `Volumes::sincronizar` sincronizando
//! todo descritor aberto. A premissa do pedido 258 -- «cinco dos oito estao
//! limpos» -- e' o que este medidor confere ANTES de qualquer conserto: o
//! `strace -f -y` decora cada descritor com o caminho, entao da' para dizer,
//! `fsync` a `fsync`, se houve `write` naquele arquivo desde o `fsync`
//! anterior. Limpo e' o arquivo que ninguem escreveu; sujo e' o que alguem
//! escreveu.
//!
//! A conta e' POR DIFERENCA, como na bancada: uma corrida de 1.000 operacoes
//! menos uma de 200, dividido por 800. O que a abertura da tabela e o primeiro
//! fecho custam cancela -- e o primeiro fecho e' justamente o que o batismo
//! (ver `Volumes::sincronizar`) paga inteiro, de proposito.
//!
//! O modo `--tempo` cronometra as mesmas operacoes no proprio processo, sem
//! traco, para o antes/depois de um conserto: medianas de `reps` corridas,
//! cada uma numa tabela recem-semeada.
//!
//! O esquema e o de `medir-crud.py`: `config(chave Str(64), valor Str(128))`,
//! indice unico `porChave`, N = 1.000 -- para o numero daqui ser o mesmo
//! trabalho que o da bancada, e nao um parente.

#[path = "apoio/strace.rs"]
mod strace;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const N: i64 = 1_000;
const OPERACOES: [&str; 3] = ["inserir", "atualizar", "excluir"];

fn esquema() -> Schema {
    Schema::new(
        "config",
        vec![
            Column::new("chave", ColumnType::Str(64)).obrigatoria(),
            Column::new("valor", ColumnType::Str(128)).obrigatoria(),
        ],
        vec![IndexDef::new("porChave", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema da bancada CRUD")
}

fn chave(i: i64) -> String {
    format!("g{}/s{}/v{i}", i % 37, i % 11)
}

fn linha(i: i64, versao: u32) -> Vec<Value> {
    vec![
        Value::Str(chave(i)),
        Value::Str(format!("valor {i:06} v{versao}")),
    ]
}

fn semear(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let mut t = Table::criar(dir, esquema()).unwrap();
    for i in 1..=N {
        t.inserir(&linha(i, 0)).unwrap();
    }
    t.sincronizar().unwrap();
}

/// O corpo: `m` operacoes de ponto, cada uma seguida do `sincronizar()` --
/// e' o que `gravar_de_verdade` faz no servidor em `por_operacao`.
fn operar(t: &mut Table, op: &str, m: i64) {
    for k in 0..m {
        match op {
            "inserir" => {
                t.inserir(&linha(N + k + 1, 0)).unwrap();
            }
            "atualizar" => {
                let rowid = (k % N + 1) as u64;
                t.atualizar(rowid, &linha(k % N + 1, 1)).unwrap();
            }
            "excluir" => {
                let rowid = (k % N + 1) as u64;
                t.excluir(rowid).unwrap();
            }
            outro => panic!("operacao desconhecida: {outro}"),
        }
        t.sincronizar().unwrap();
    }
}

/// O corpo tracado: abre a tabela semeada e opera. Nada antes, nada depois.
fn sonda(dir: &str, op: &str, m: i64) {
    let mut t = Table::abrir(dir, "config").expect("abrir a tabela semeada");
    operar(&mut t, op, m);
}

/// Extensao do arquivo, que e' o nome que a casa usa para cada peca.
fn peca(caminho: &str) -> String {
    Path::new(caminho)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_else(|| "?".to_string())
}

/// `(fsync sujos, fsync limpos)` por peca, numa corrida de `m` operacoes.
fn contar(dir: &Path, op: &str, m: i64, log: &Path) -> Option<BTreeMap<String, (u64, u64)>> {
    let eu = std::env::current_exe().ok()?;
    let args = [
        "--sonda".to_string(),
        dir.display().to_string(),
        op.to_string(),
        m.to_string(),
    ];
    strace::tracar(&eu, &args, "fsync,write,pwrite64", log)?;
    let mut sujo: BTreeMap<String, bool> = BTreeMap::new();
    let mut saida: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for (chamada, alvo) in strace::chamadas(log) {
        match chamada.as_str() {
            "write" | "pwrite64" => {
                sujo.insert(alvo, true);
            }
            "fsync" => {
                let estava_sujo = sujo.insert(alvo.clone(), false).unwrap_or(false);
                let e = saida.entry(peca(&alvo)).or_insert((0, 0));
                if estava_sujo {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
            _ => {}
        }
    }
    Some(saida)
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Cronometra as operacoes no proprio processo: mediana de `reps` corridas
/// de `m` operacoes, cada corrida numa tabela recem-semeada. A tabela e'
/// semeada e sincronizada FORA do cronometro, e a primeira operacao paga o
/// primeiro fecho do processo -- que e' o que acontece no servidor tambem.
fn tempo(base: &Path, reps: usize, m: i64) {
    println!(
        "=== por_operacao: microssegundos por operacao (N = {N}, {m} ops, mediana de {reps}) ===\n"
    );
    for op in OPERACOES {
        let mut corridas = Vec::with_capacity(reps);
        for r in 0..reps {
            let dir = base.join(format!("tempo-{op}-{r}"));
            semear(&dir);
            let mut t = Table::abrir(&dir, "config").unwrap();
            let inicio = Instant::now();
            operar(&mut t, op, m);
            corridas.push(inicio.elapsed().as_secs_f64() * 1e6 / m as f64);
            drop(t);
            let _ = std::fs::remove_dir_all(&dir);
        }
        let min = corridas.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = corridas.iter().cloned().fold(0.0, f64::max);
        println!(
            "  {op:<10} {:>9.1} us/op   (min {min:.1}, max {max:.1})",
            mediana(corridas)
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--sonda") {
        sonda(&args[i + 1], &args[i + 2], args[i + 3].parse().unwrap());
        return;
    }
    // O mesmo cache quente da bancada CRUD.
    phxsql_store::ndx::definir_cache_paginas(1_000_000);
    // O padrao de fabrica: a lixeira espera o disco por exclusao, e o
    // `sincronizar()` de depois espera de novo. E' o que o servidor faz.
    phxsql_store::lixeira::definir_na_janela(false);

    let base: PathBuf =
        std::env::temp_dir().join(format!("phx-fsync-por-op-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();

    if args.iter().any(|a| a == "--tempo") {
        let reps: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
        let m: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1_000);
        tempo(&base, reps, m);
        let _ = std::fs::remove_dir_all(&base);
        return;
    }

    println!(
        "=== fsync por operacao em `por_operacao`, por arquivo, sujo x limpo (N = {N}, 1.000 - 200 ops) ===\n"
    );
    let mut pior = 0.0f64;
    for op in OPERACOES {
        let mut por_m: Vec<BTreeMap<String, (u64, u64)>> = Vec::new();
        for m in [200i64, 1_000] {
            let dir = base.join(format!("{op}-{m}"));
            semear(&dir);
            let log = base.join(format!("strace-{op}-{m}.log"));
            match contar(&dir, op, m, &log) {
                Some(c) => por_m.push(c),
                None => {
                    println!("  sem `strace` nesta maquina -- a contagem nao se substitui.");
                    let _ = std::fs::remove_dir_all(&base);
                    std::process::exit(2);
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        let (pequena, grande) = (&por_m[0], &por_m[1]);
        let mut total_sujo = 0.0;
        let mut total_limpo = 0.0;
        println!("  {op}:");
        println!("    {:<9} {:>8} {:>8}", "arquivo", "sujos", "limpos");
        for (p, (s1, l1)) in grande {
            let (s0, l0) = pequena.get(p).copied().unwrap_or((0, 0));
            let sujos = (*s1 as f64 - s0 as f64) / 800.0;
            let limpos = (*l1 as f64 - l0 as f64) / 800.0;
            total_sujo += sujos;
            total_limpo += limpos;
            println!("    {p:<9} {sujos:>8.2} {limpos:>8.2}");
        }
        println!(
            "    {:<9} {total_sujo:>8.2} {total_limpo:>8.2}   = {:.2} fsync por {op}, {:.0}% em arquivo limpo\n",
            "TOTAL",
            total_sujo + total_limpo,
            total_limpo / (total_sujo + total_limpo).max(1e-9) * 100.0
        );
        pior = pior.max(total_sujo + total_limpo);
    }
    println!("  (limpo = nenhum `write` naquele arquivo desde o `fsync` anterior do traco)");
    println!("  pior operacao: {pior:.2} fsync");
    let _ = std::fs::remove_dir_all(&base);
}
