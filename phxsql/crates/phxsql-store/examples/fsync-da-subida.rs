//! Quantos `fsync` custa a SUBIDA do byte 52 numa janela -- medido, nao
//! contado no fonte. Pedido 533.
//!
//! ```bash
//! cargo run --release --example fsync-da-subida -p phxsql-store
//! cargo run --release --example fsync-da-subida -p phxsql-store -- --numeros
//! cargo run --release --example fsync-da-subida -p phxsql-store -- 1 5000
//! ```
//!
//! # Por que ele existe, ao lado do `fsync-por-fecho`
//!
//! Porque as duas catracas de `fsync` que ja existiam sao CEGAS a este, e isso
//! esta medido (`docs/propostas/pesquisa-rodada-2026-09-24.md` §1.3): o
//! `fsync-por-fecho` conta so' o que vem depois do marco do fecho (8 -> 8 com
//! a subida sincronizada), e o `alcancam-fsync-2` do mapa da trava conta
//! SECOES que alcancam `fsync`, e as quatro que passaram a alcancar pela
//! subida ja estavam nas 23. Um segundo `fsync` na subida -- o conserto
//! ingenuo de sincronizar a cada pagina suja, por exemplo -- entraria calado
//! nas duas. Esta e a catraca dele.
//!
//! # Como ele mede
//!
//! A semeadura cria a tabela e fecha a janela (`sincronizar`: byte 52 em 0 no
//! disco). O binario se **reexecuta** com `--sonda <dir> <linhas>` sob
//! `strace -f -y -e trace=fsync,fdatasync,getppid`: o filho reabre a tabela,
//! faz um `getppid` de marco, insere `linhas` linhas SEM fechar a janela --
//! o que o `gravar_de_verdade` do servidor faz entre dois fechos --, faz outro
//! `getppid` e sai. So' os `fsync`/`fdatasync` entre os dois marcos contam: a
//! abertura antes e o `Drop` depois nao sao a janela.
//!
//! Tres escalas, pela mesma razao do irmao: um numero medido uma vez so' nao
//! separa «e' uma vez por janela» de «e' proporcional a algo que cresceu». A
//! subida e' UMA por passagem de 0 para 1, e numa janela sem `sincronizar` no
//! meio isso e' uma so, entao as tres batem no mesmo numero -- e o dia em que
//! nao baterem, o relatorio mostra. Onde quem chama sincroniza sozinho (a
//! cascata, o `por_operacao`) a passagem se repete a cada `sincronizar`: ver o
//! `docs/FORMATO.md`, parecer do DBA sobre o 533, C3.

use std::path::Path;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::conferidor_fsync::TETO_FSYNC_DA_SUBIDA;
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

/// O corpo tracado: a janela aberta entre dois marcos.
fn sonda(dir: &str, linhas: i64) {
    let mut t = Table::abrir(dir, "sonda").expect("abrir a tabela semeada");
    #[cfg(unix)]
    let _ = std::os::unix::process::parent_id();
    for i in 1..=linhas {
        t.inserir(&linha(1_000_000 + i)).expect("inserir na janela");
    }
    #[cfg(unix)]
    let _ = std::os::unix::process::parent_id();
    // De proposito sem sincronizar: o fecho e o outro medidor.
}

/// A tabela com uma linha, e a janela FECHADA: o byte 52 em 0 no disco.
fn semear(dir: &Path) {
    let mut t = Table::criar(dir, esquema()).unwrap();
    t.inserir(&linha(1)).unwrap();
    t.sincronizar().unwrap();
}

/// Roda a sonda sob `strace` e devolve (fsyncs na janela, quantos no `.ndx`).
///
/// `None` quando esta maquina nao tem `strace`: `fsync` que aconteceu ou nao
/// e' fato do sistema operacional, e teste unitario nao o observa.
fn contar(dir: &Path, linhas: i64, log: &Path) -> Option<(usize, usize)> {
    std::process::Command::new("strace")
        .arg("-V")
        .output()
        .ok()?;
    let eu = std::env::current_exe().ok()?;
    let saida = std::process::Command::new("strace")
        .args(["-f", "-y", "-e", "trace=fsync,fdatasync,getppid", "-o"])
        .arg(log)
        .arg(&eu)
        .arg("--sonda")
        .arg(dir)
        .arg(linhas.to_string())
        .output()
        .ok()?;
    if !saida.status.success() {
        eprintln!("a sonda falhou: {}", String::from_utf8_lossy(&saida.stderr));
        return None;
    }
    let texto = std::fs::read_to_string(log).ok()?;
    let marcos: Vec<usize> = texto
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("getppid("))
        .map(|(i, _)| i)
        .collect();
    let [ini, fim] = marcos[..] else {
        eprintln!(
            "o traco tem {} marcos getppid, e a conta precisa de dois: nao sabe \
             onde a janela comeca e acaba",
            marcos.len()
        );
        return None;
    };
    let janela: Vec<&str> = texto.lines().skip(ini + 1).take(fim - ini - 1).collect();
    let e_sync = |l: &str| l.contains("fsync(") || l.contains("fdatasync(");
    let total = janela.iter().filter(|l| e_sync(l)).count();
    let no_ndx = janela
        .iter()
        .filter(|l| e_sync(l) && l.contains(".ndx>"))
        .count();
    Some((total, no_ndx))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--sonda") {
        sonda(&args[i + 1], args[i + 2].parse().expect("linhas na janela"));
        return;
    }
    let escalas: Vec<i64> = {
        let dados: Vec<i64> = args.iter().filter_map(|a| a.parse().ok()).collect();
        if dados.is_empty() {
            // 10.000 e nao mais: o filho roda sob `strace` sem filtro de
            // seccomp (o que funciona em todo `strace`), e cada chamada de
            // sistema para o processo -- 20.000 linhas custavam 10 s no perfil
            // de teste, e a conta que importa (uma por janela) ja se ve em
            // dezenas de paginas sujas.
            vec![1, 1_000, 10_000]
        } else {
            dados
        }
    };

    let base = std::env::temp_dir().join(format!("phx-subida-{}", std::process::id()));
    let mut medidas: Vec<(i64, usize, usize)> = Vec::new();
    for linhas in &escalas {
        let dir = base.join(format!("e{linhas}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        semear(&dir);
        let Some((total, no_ndx)) =
            contar(&dir, *linhas, &base.join(format!("strace-{linhas}.log")))
        else {
            eprintln!(
                "strace nao esta instalado nesta maquina (ou o traco nao tem os \
                 marcos) -- esta catraca so' se mede contra o sistema \
                 operacional de verdade. Instale o strace."
            );
            let _ = std::fs::remove_dir_all(&base);
            std::process::exit(2);
        };
        medidas.push((*linhas, total, no_ndx));
    }
    let _ = std::fs::remove_dir_all(&base);

    let pior = medidas.iter().map(|m| m.1).max().unwrap_or(0);
    println!("SUBIDA DO BYTE 52 — quantos `fsync` uma janela custa, por tabela\n");
    println!("  linhas na janela   fsync   dos quais no .ndx");
    for (linhas, total, no_ndx) in &medidas {
        println!("  {linhas:>16}   {total:>5}   {no_ndx:>17}");
    }
    println!("\n  pior caso .............. {pior}");
    println!("  catraca (so desce) ..... {TETO_FSYNC_DA_SUBIDA}");
    if medidas.iter().any(|m| m.2 == 0) {
        println!(
            "\n  ATENCAO: alguma escala abriu a janela sem `fsync` no `.ndx`: a \
             subida do byte 52 voltou a ficar so no cache (pedido 533)."
        );
    }

    // SAIDA DE MAQUINA -- o mesmo motivo do irmao `fsync-por-fecho.rs`.
    if args.iter().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_FSYNC_DA_SUBIDA;\
             onde=crates/phxsql-store/src/conferidor_fsync.rs;\
             valor={TETO_FSYNC_DA_SUBIDA};medido={pior};\
             mede=fsync gastos pela subida do byte 52 numa janela, por tabela"
        );
    }
}
