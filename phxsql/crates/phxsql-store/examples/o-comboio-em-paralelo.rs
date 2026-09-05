//! O comboio do fecho de janela: o `fsync` das K tabelas em SERIE contra em PARALELO.
//!
//! ```bash
//! cargo run --release --example o-comboio-em-paralelo -p phxsql-store -- [K_max] [linhas] [janelas]
//! cargo run --release --example o-comboio-em-paralelo -p phxsql-store -- --contar
//! ```
//!
//! ## Por que este medidor existe
//!
//! A §12 do `docs/CONCORRENCIA.md` mediu o comboio POR FORA (o p99 do leitor
//! sobe 2,01x de K=1 para K=4) e o `o-comboio-por-dentro` o dividiu POR DENTRO:
//! `abrir` 5-7%, `fsync` 93-96%. O veredito impresso por aquele medidor manda
//! soltar a trava entre uma tabela e a seguinte -- e isso mexe na ordem que o
//! comentario do laco diz que «nao se inverte».
//!
//! Antes de mexer na ordem, ha uma pergunta mais barata que ninguem tinha
//! feito: **o `K x fsync` precisa mesmo ser em serie?** Sincronizar as K
//! tabelas ao mesmo tempo nao encosta na ordem nenhuma -- o encontro continua
//! atomico sob a mesma trava, so' o miolo dele deixa de ser um laco. Se o
//! sistema de arquivos junta as confirmacoes (o `ext4` junta o diario), o
//! comboio encolhe sem risco de durabilidade nenhum.
//!
//! *Medir a premissa do item vem antes de implementar o item* -- e aqui o item
//! e' nosso.
//!
//! ## O que ele mede, e o que ele NAO mede
//!
//! Mede o TEMPO DE TRABALHO do fecho -- o que a trava global fica presa --,
//! nos dois arranjos, alternando os dois dentro da mesma corrida para que uma
//! variacao da maquina caia nos dois lados. Nao mede espera de cliente
//! nenhum: e' um processo so', sem rede e sem trava. E o `--contar` responde a
//! outra metade, que nao depende de relogio: **o arranjo muda o numero de
//! `fsync`?** Se mudasse, o ganho seria ilusao -- seria durabilidade a menos.

use std::path::{Path, PathBuf};
use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Table;

fn esquema(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
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
        Value::Str("Blumenau".into()),
    ]
}

/// Uma janela de escrita: uma linha em cada tabela, SEM sincronizar -- que e'
/// o que `gravar_de_verdade` faz enquanto a janela nao fecha.
fn sujar(db: &phxsql_store::catalogo::Database, nomes: &[String], id: i64) {
    for n in nomes {
        let mut t = db.abrir_qualificada(n).unwrap();
        t.inserir(&linha(id)).unwrap();
    }
}

/// O fecho de HOJE: abre e sincroniza uma tabela de cada vez.
fn fechar_em_serie(inst: &Instancia, nomes: &[String]) {
    for n in nomes {
        let mut t = inst
            .abrir_database("bancada")
            .and_then(|d| d.abrir_qualificada(n))
            .unwrap();
        t.sincronizar().unwrap();
    }
}

/// O fecho proposto: abre as K em serie (sao 5-7% do custo, e o catalogo tem
/// estado compartilhado) e sincroniza as K ao mesmo tempo.
///
/// A ordem DENTRO de cada tabela nao muda -- `Table::sincronizar` continua
/// inteiro, com o `.trash` antes do `.reg`. O que deixa de existir e' a ordem
/// ENTRE tabelas, e nao havia ordem entre tabelas para preservar: o encontro
/// so' termina quando todas terminam.
fn fechar_em_paralelo(inst: &Instancia, nomes: &[String]) {
    let mut abertas: Vec<Table> = Vec::with_capacity(nomes.len());
    for n in nomes {
        abertas.push(
            inst.abrir_database("bancada")
                .and_then(|d| d.abrir_qualificada(n))
                .unwrap(),
        );
    }
    std::thread::scope(|escopo| {
        let mut fios = Vec::with_capacity(abertas.len());
        for t in abertas.iter_mut() {
            fios.push(escopo.spawn(move || t.sincronizar()));
        }
        for f in fios {
            f.join().unwrap().unwrap();
        }
    });
}

/// Monta a base com K tabelas semeadas.
fn montar(base: &Path, k: usize, semear: i64) -> (Instancia, Vec<String>) {
    let _ = std::fs::remove_dir_all(base);
    let inst = Instancia::nova(base).unwrap();
    let db = inst.criar_database("bancada").unwrap();
    let nomes: Vec<String> = (0..k).map(|i| format!("tab{i:02}")).collect();
    for n in &nomes {
        let mut t = Table::criar(db.caminho(), esquema(n)).unwrap();
        for i in 1..=semear {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    (inst, nomes)
}

/// O corpo tracado pelo `--contar`: so' o fecho, nada antes e nada depois.
fn sonda(dir: &str, k: usize, paralelo: bool) {
    let inst = Instancia::nova(Path::new(dir)).unwrap();
    let nomes: Vec<String> = (0..k).map(|i| format!("tab{i:02}")).collect();
    if paralelo {
        fechar_em_paralelo(&inst, &nomes);
    } else {
        fechar_em_serie(&inst, &nomes);
    }
}

/// Conta os `fsync` de um fecho de K tabelas nos dois arranjos.
///
/// Roda a sonda num processo FILHO sob `strace`: a semeadura dentro do mesmo
/// traco foi o erro que o `fsync-por-fecho` ja registrou, e ele nao se repete
/// aqui. `None` quando esta maquina nao tem `strace` -- um `fsync` que
/// aconteceu ou nao e' fato do sistema operacional, e teste unitario nao o ve.
fn contar(dir: &Path, k: usize, paralelo: bool, log: &Path) -> Option<usize> {
    std::process::Command::new("strace")
        .arg("-V")
        .output()
        .ok()?;
    let eu = std::env::current_exe().ok()?;
    let saida = std::process::Command::new("strace")
        .args(["-f", "-y", "-e", "trace=fsync", "-o"])
        .arg(log)
        .arg(&eu)
        .arg("--sonda")
        .arg(dir)
        .arg(k.to_string())
        .arg(if paralelo { "paralelo" } else { "serie" })
        .output()
        .ok()?;
    if !saida.status.success() {
        eprintln!("a sonda falhou: {}", String::from_utf8_lossy(&saida.stderr));
        return None;
    }
    let texto = std::fs::read_to_string(log).ok()?;
    Some(texto.lines().filter(|l| l.contains("fsync(")).count())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--sonda") {
        let k: usize = args[i + 2].parse().unwrap();
        sonda(&args[i + 1], k, args[i + 3] == "paralelo");
        return;
    }

    let base: PathBuf =
        std::env::temp_dir().join(format!("phx-comboio-par-{}", std::process::id()));

    if args.iter().any(|a| a == "--contar") {
        let k = 4usize;
        println!("=== o arranjo muda o numero de `fsync`? (K={k}) ===\n");
        let log = base.with_extension("strace");
        let mut respostas = Vec::new();
        for paralelo in [false, true] {
            let (_i, _n) = montar(&base, k, 200);
            // deixa as K sujas, como o fecho as encontra
            {
                let inst = Instancia::nova(&base).unwrap();
                let db = inst.abrir_database("bancada").unwrap();
                let nomes: Vec<String> = (0..k).map(|i| format!("tab{i:02}")).collect();
                sujar(&db, &nomes, 999_999);
            }
            match contar(&base, k, paralelo, &log) {
                Some(n) => respostas.push((paralelo, n)),
                None => {
                    // 2 = esta maquina nao tem `strace`, e nao ha substituto:
                    // `fsync` que aconteceu ou nao e' fato do sistema
                    // operacional, e teste unitario nao o observa. Quem cobra
                    // este medidor distingue "nao pude medir" de "medi e
                    // reprovou" pelo codigo de saida.
                    println!("  sem `strace` nesta maquina -- a contagem nao se substitui.");
                    let _ = std::fs::remove_dir_all(&base);
                    std::process::exit(2);
                }
            }
        }
        let _ = std::fs::remove_file(&log);
        let _ = std::fs::remove_dir_all(&base);
        for (paralelo, n) in &respostas {
            println!(
                "  {:<10} {n} fsync ({:.1} por tabela)",
                if *paralelo { "paralelo" } else { "serie" },
                *n as f64 / k as f64
            );
        }
        let iguais = respostas[0].1 == respostas[1].1;
        println!(
            "\n  veredito: {}",
            if iguais {
                "IGUAIS -- o arranjo nao compra tempo vendendo durabilidade."
            } else {
                "DIFERENTES -- o ganho seria fsync a menos, e isso nao e' ganho."
            }
        );
        std::process::exit(if iguais { 0 } else { 1 });
    }

    let mut arg = args.iter().filter_map(|s| s.parse::<usize>().ok());
    let k_max: usize = arg.next().unwrap_or(16);
    let semear: i64 = arg.next().unwrap_or(2_000) as i64;
    let janelas: usize = arg.next().unwrap_or(30);

    let (inst, nomes) = montar(&base, k_max, semear);
    let db = inst.abrir_database("bancada").unwrap();

    println!("=== o comboio em serie contra em paralelo ===");
    println!(
        "    {k_max} tabelas, {semear} linhas semeadas em cada, {janelas} janelas por arranjo\n"
    );
    println!(
        "  {:>3}  {:>12}  {:>12}  {:>8}",
        "K", "serie (us)", "paralelo(us)", "ganho"
    );
    println!("  {}", "-".repeat(42));

    let mut proxima: i64 = semear;
    let mut tabela = Vec::new();
    let mut k = 1usize;
    while k <= k_max {
        // aquece: a primeira janela de cada K paga cache frio de diretorio.
        for _ in 0..2 {
            proxima += 1;
            sujar(&db, &nomes[..k], proxima);
            fechar_em_serie(&inst, &nomes[..k]);
            proxima += 1;
            sujar(&db, &nomes[..k], proxima);
            fechar_em_paralelo(&inst, &nomes[..k]);
        }
        let mut serie = 0.0f64;
        let mut paralelo = 0.0f64;
        // ALTERNADO, e nao um bloco de cada: uma variacao da maquina no meio
        // da corrida cairia inteira num dos lados e viraria "ganho".
        for _ in 0..janelas {
            proxima += 1;
            sujar(&db, &nomes[..k], proxima);
            let t0 = Instant::now();
            fechar_em_serie(&inst, &nomes[..k]);
            serie += t0.elapsed().as_secs_f64() * 1e6;

            proxima += 1;
            sujar(&db, &nomes[..k], proxima);
            let t0 = Instant::now();
            fechar_em_paralelo(&inst, &nomes[..k]);
            paralelo += t0.elapsed().as_secs_f64() * 1e6;
        }
        let s = serie / janelas as f64;
        let p = paralelo / janelas as f64;
        println!("  {k:>3}  {s:>12.1}  {p:>12.1}  {:>7.2}x", s / p);
        tabela.push((k, s, p));
        k *= 2;
    }

    let _ = std::fs::remove_dir_all(&base);

    println!("\n=== o veredito ===\n");
    let (kn, sn, pn) = *tabela.last().unwrap();
    let ganho = sn / pn;
    println!("  Em K={kn} o fecho vai de {sn:.0} us para {pn:.0} us: {ganho:.2}x.");
    if ganho >= 1.5 {
        println!("  O `fsync` das K tabelas JUNTA no sistema de arquivos. O comboio encolhe");
        println!("  sem soltar a trava e sem inverter a ordem do group commit: o encontro");
        println!("  continua atomico, so' o miolo dele deixa de ser um laco.");
    } else {
        println!("  O `fsync` das K tabelas NAO junta nesta maquina. Paralelizar nao compra");
        println!("  o comboio, e quem quiser compra-lo tem de soltar a trava -- decisao de");
        println!("  durabilidade, com a matriz de queda na mao.");
    }
}
