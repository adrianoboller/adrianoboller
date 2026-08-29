//! Quanto o `.log` custa numa insercao, e o que um buffer compraria.
//!
//! ```bash
//! cargo run --release --example custo-do-log -- [eventos]
//! ```
//!
//! Os 5,4 us de `.reg` + `.log` por linha foram medidos JUNTOS e nunca
//! decompostos -- e sem decompor nao da para responder "vale a pena guardar o
//! diario em memoria e gravar de uma vez?". Este medidor separa.
//!
//! Tres medidas:
//!
//! 1. `registrar` de verdade, evento por evento, como a insercao chama;
//! 2. o mesmo com a imagem da linha junto, que e o modo da replicacao;
//! 3. e, com escritas cruas no mesmo arquivo, a diferenca entre gravar o
//!    cabecalho A CADA evento -- o que ele faz hoje -- e gravar uma vez so no
//!    fim, que e o TETO do que um buffer compraria.
//!
//! A terceira e a que importa para a decisao, porque separa duas coisas que a
//! pergunta junta: parar de reescrever o CABECALHO (que nao precisa de buffer
//! nenhum) e segurar os EVENTOS em RAM (que troca uma garantia).

use std::io::{Seek, SeekFrom, Write};
use std::time::Instant;

use phxsql_core::paginacao::Paginacao;
use phxsql_store::log::{LogFile, Operacao};

/// Escritas cruas num arquivo, para separar o custo da CHAMADA do resto.
fn cru(rotulo: &str, n: u64, cabecalho_por_evento: bool) -> f64 {
    let caminho = std::env::temp_dir().join(format!("phx-log-cru-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_file(&caminho);
    let mut f = std::fs::File::create(&caminho).unwrap();
    let evento = [0u8; 44];
    let cabecalho = [0u8; 64];
    f.write_all(&cabecalho).unwrap();

    let inicio = Instant::now();
    let mut fim = 64u64;
    for _ in 0..n {
        f.seek(SeekFrom::Start(fim)).unwrap();
        f.write_all(&evento).unwrap();
        fim += 44;
        if cabecalho_por_evento {
            f.seek(SeekFrom::Start(0)).unwrap();
            f.write_all(&cabecalho).unwrap();
        }
    }
    if !cabecalho_por_evento {
        f.seek(SeekFrom::Start(0)).unwrap();
        f.write_all(&cabecalho).unwrap();
    }
    let s = inicio.elapsed().as_secs_f64();
    drop(f);
    let _ = std::fs::remove_file(&caminho);
    s * 1e6 / n as f64
}

fn medir(rotulo: &str, n: u64, imagem: &[u8]) -> f64 {
    let dir = std::env::temp_dir().join(format!("phx-log-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut log = LogFile::criar(&dir, "t", Paginacao::default()).unwrap();

    let inicio = Instant::now();
    for i in 1..=n {
        log.registrar_com_imagem(Operacao::Inclusao, i, 1, imagem)
            .unwrap();
    }
    log.sincronizar().unwrap();
    let s = inicio.elapsed().as_secs_f64();

    let _ = std::fs::remove_dir_all(&dir);
    let us = s * 1e6 / n as f64;
    println!("  {rotulo:<34} {us:>6.2} us por evento");
    us
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    println!("=== o que o `.log` custa por evento, em {n} eventos ===\n");
    let sem = medir("registrar, sem imagem", n, &[]);
    let com = medir("registrar, com imagem (179 B)", n, &[0u8; 179]);

    println!("\n=== o teto de um buffer: escritas cruas, mesmo arquivo ===\n");
    let hoje = cru("hoje", n, true);
    let uma = cru("uma-vez", n, false);
    println!("  evento + cabecalho a cada um (hoje)  {hoje:>6.2} us por evento");
    println!("  evento, e o cabecalho uma vez so     {uma:>6.2} us por evento");
    println!(
        "\n  A segunda escrita -- a do cabecalho -- custa {:.2} us por evento.",
        hoje - uma
    );

    println!("\n=== o que isso significa numa insercao de 17,0 us ===\n");
    println!(
        "  o `.log` inteiro, sem imagem ......... {sem:>6.2} us   {:>5.1}%",
        sem / 17.0 * 100.0
    );
    println!(
        "  o `.log` inteiro, com imagem ......... {com:>6.2} us   {:>5.1}%",
        com / 17.0 * 100.0
    );
    println!(
        "  so a reescrita do cabecalho .......... {:>6.2} us   {:>5.1}%",
        hoje - uma,
        (hoje - uma) / 17.0 * 100.0
    );
    println!(
        "\n  Guardar os EVENTOS em RAM compraria, no maximo, os {sem:.2} us do\n  \
         diario inteiro -- e trocaria a garantia de que linha gravada tem\n  \
         evento gravado. Parar de reescrever o CABECALHO compraria {:.2} us\n  \
         sem buffer nenhum.",
        hoje - uma
    );
}
