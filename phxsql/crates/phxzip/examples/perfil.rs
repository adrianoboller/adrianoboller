//! Compacta os arquivos dados num fio so, uma vez, para o `callgrind` ou o
//! `perf` medirem onde vai o tempo do codificador.
//!   cargo run --release -p phxzip --example perfil -- NIVEL ARQ...
fn main() {
    let mut args = std::env::args().skip(1);
    let nivel: u8 = args.next().and_then(|a| a.parse().ok()).unwrap_or(5);
    let mut dados = Vec::new();
    for a in args {
        dados.extend(std::fs::read(&a).expect("ler"));
    }
    let t = std::time::Instant::now();
    let (_, c) = phxzip::lzma::codificar_lzma2(&dados, phxzip::lzma::Nivel::de(nivel));
    eprintln!(
        "{} -> {} bytes em {:.3} s",
        dados.len(),
        c.len(),
        t.elapsed().as_secs_f64()
    );
}
