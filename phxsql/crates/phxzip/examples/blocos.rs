//! Mede a compactacao em blocos: tamanho e tempo por tamanho de bloco e por
//! numero de fios, sobre os arquivos dados na linha de comando.
//!   cargo run --release -p phxzip --features std --example blocos -- NIVEL ARQ...
use std::time::Instant;

use phxzip::{Escritor, Opcoes};

fn main() {
    let mut args = std::env::args().skip(1);
    let nivel: u8 = args.next().and_then(|a| a.parse().ok()).unwrap_or(5);
    let arquivos: Vec<(String, Vec<u8>)> = args
        .map(|a| (a.clone(), std::fs::read(&a).expect("ler")))
        .collect();
    let total: usize = arquivos.iter().map(|(_, d)| d.len()).sum();
    let nucleos = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!("nivel {nivel}, {total} bytes, {nucleos} nucleos");
    let medir = |fios: usize, bloco: usize| {
        let mut melhor = f64::MAX;
        let mut tam = 0;
        for _ in 0..3 {
            let mut e = Escritor::novo(Opcoes {
                nivel,
                fios,
                bloco,
                ..Opcoes::default()
            });
            for (n, d) in &arquivos {
                e.arquivo(n.rsplit('/').next().unwrap(), d.clone(), None, None)
                    .unwrap();
            }
            let t = Instant::now();
            tam = e.gravar().unwrap().len();
            melhor = melhor.min(t.elapsed().as_secs_f64());
        }
        (tam, melhor)
    };
    let (base_t, base_s) = medir(1, 0);
    println!("um bloco, 1 fio        {base_t:>10} bytes  {base_s:>7.3} s");
    for mib in [1usize, 4, 8, 16, 1024] {
        let (t, s) = medir(nucleos, mib << 20);
        println!(
            "bloco {mib:>2} MiB, {nucleos} fios  {t:>10} bytes  {s:>7.3} s   tamanho {:+.2}%  tempo {:.2}x mais rapido",
            (t as f64 / base_t as f64 - 1.0) * 100.0,
            base_s / s
        );
    }
}
