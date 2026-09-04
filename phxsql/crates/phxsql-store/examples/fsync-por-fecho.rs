//! Quantos `fsync` custa um FECHO DE JANELA -- medido, nao contado no fonte.
//!
//! ```bash
//! cargo run --release --example fsync-por-fecho -p phxsql-store
//! cargo run --release --example fsync-por-fecho -p phxsql-store -- --numeros
//! cargo run --release --example fsync-por-fecho -p phxsql-store -- 20 2000
//! ```
//!
//! # Por que ele existe, e nao so' o teste ao lado
//!
//! Porque `docs/qa/medir.py` monta o inventario das catracas **perguntando** a
//! quem imprime `catraca:` e responde a `--numeros`, varrendo
//! `crates/*/examples/*.rs`. Uma catraca que vive so' dentro de um `tests/*.rs`
//! nao aparece no inventario -- e pelo criterio escrito no proprio
//! `docs/CATRACAS.md` isso a torna promessa, e nao catraca. Este exemplo e' a
//! catraca SE DESCREVENDO no formato que a casa ja usa; o teste
//! `tests/catraca-fsync-por-fecho.rs` roda este binario e cobra o numero.
//!
//! # Como ele mede
//!
//! O fecho de janela e' `descarregar_sujas_com`, no servidor: quem escreveu foi
//! uma `Table` que ja morreu, e quem sincroniza e' uma `Table` REABERTA so'
//! para isso. A semeadura reproduz esse estado (grava e larga sem sincronizar)
//! e o binario se **reexecuta** com `--sonda <dir>` sob
//! `strace -f -y -e trace=fsync`. So' o processo filho e' tracado, e o corpo
//! dele e' o fecho e nada mais -- contar `fsync` com a semeadura dentro do
//! mesmo traco foi o erro que quase saiu na primeira leitura deste numero.
//!
//! Tres escalas, e nao uma: um numero medido uma vez so' nao separa "e' o
//! caminho fixo de arquivos" de "e' proporcional a algo que cresceu no teste".
//! O fecho e' POR ARQUIVO, entao as tres tem de bater no mesmo numero -- e o
//! dia em que nao baterem, a suposicao caiu e o relatorio mostra.

use std::path::Path;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::conferidor_fsync::TETO_FSYNC_POR_FECHO_V2;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "sonda",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("nome {i}"))]
}

/// O corpo tracado: reabre e fecha a janela. Nada antes, nada depois.
fn sonda(dir: &str) {
    let mut t = Table::abrir(dir, "sonda").expect("abrir a tabela semeada");
    t.sincronizar().expect("fechar a janela");
}

/// Deixa a tabela no estado que o fecho da janela encontra: escrita por uma
/// instancia que morreu sem sincronizar.
fn semear(dir: &Path, linhas: i64) {
    let mut t = Table::criar(dir, esquema()).unwrap();
    for i in 1..=linhas {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let mut t = Table::abrir(dir, "sonda").unwrap();
    t.inserir(&linha(linhas + 1)).unwrap();
    // De proposito sem sincronizar -- e' o que `gravar_de_verdade` faz quando
    // a janela ainda nao fechou.
}

/// Roda a sonda sob `strace` e devolve (fsyncs, quantos tocaram um `.reg`).
///
/// `None` quando esta maquina nao tem `strace`: sem o sistema operacional de
/// verdade nao ha substituto -- um `fsync` que aconteceu ou nao e' fato do SO,
/// e teste unitario nao o observa.
fn contar(dir: &Path, log: &Path) -> Option<(usize, usize)> {
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
        .output()
        .ok()?;
    if !saida.status.success() {
        eprintln!("a sonda falhou: {}", String::from_utf8_lossy(&saida.stderr));
        return None;
    }
    let texto = std::fs::read_to_string(log).ok()?;
    let total = texto.lines().filter(|l| l.contains("fsync(")).count();
    let no_reg = texto
        .lines()
        .filter(|l| l.contains("fsync(") && l.contains(".reg>"))
        .count();
    Some((total, no_reg))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--sonda") {
        sonda(&args[i + 1]);
        return;
    }
    let escalas: Vec<i64> = {
        let dados: Vec<i64> = args.iter().filter_map(|a| a.parse().ok()).collect();
        if dados.is_empty() {
            vec![20, 2_000, 200_000]
        } else {
            dados
        }
    };

    let base = std::env::temp_dir().join(format!("phx-fecho-{}", std::process::id()));
    let mut medidas: Vec<(i64, usize, usize)> = Vec::new();
    for linhas in &escalas {
        let dir = base.join(format!("e{linhas}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        semear(&dir, *linhas);
        let Some((total, no_reg)) = contar(&dir, &base.join(format!("strace-{linhas}.log"))) else {
            eprintln!(
                "strace nao esta instalado nesta maquina -- esta catraca so' se \
                 mede contra o sistema operacional de verdade. Instale o strace."
            );
            let _ = std::fs::remove_dir_all(&base);
            std::process::exit(2);
        };
        medidas.push((*linhas, total, no_reg));
    }
    let _ = std::fs::remove_dir_all(&base);

    let pior = medidas.iter().map(|m| m.1).max().unwrap_or(0);
    println!("FECHO DE JANELA — quantos `fsync` ele custa\n");
    println!("  linhas semeadas   fsync   dos quais no .reg");
    for (linhas, total, no_reg) in &medidas {
        println!("  {linhas:>15}   {total:>5}   {no_reg:>17}");
    }
    println!("\n  pior caso .............. {pior}");
    println!("  catraca (so desce) ..... {TETO_FSYNC_POR_FECHO_V2}");
    if medidas.iter().any(|m| m.2 == 0) {
        println!(
            "\n  ATENCAO: alguma escala fechou a janela sem tocar o `.reg`. \
             E' o defeito que a guarda `fecho-da-janela-sincroniza-o-reg` cobra."
        );
    }

    // SAIDA DE MAQUINA -- o mesmo motivo do irmao `textos-fora-da-fabrica.rs`:
    // gerador que le prosa publica o numero de ontem quando a prosa muda.
    if args.iter().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_FSYNC_POR_FECHO_V2;\
             onde=crates/phxsql-store/src/conferidor_fsync.rs;\
             valor={TETO_FSYNC_POR_FECHO_V2};medido={pior};\
             mede=fsync gastos por fecho de janela de durabilidade"
        );
    }
}
