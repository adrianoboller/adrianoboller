//! Mede a vazao das duas AEAD do registro TLS (pedido 572, T5): o AES-128-GCM
//! de tempo constante desta casa contra o ChaCha20-Poly1305.
//!
//! `cargo run --release --example vazao-aead -p phxsql-core` cifra registros
//! cheios (2^14 bytes) e imprime MiB/s de cada uma, mediana de 5 rodadas.

use std::time::Instant;

use phxsql_core::tls13::{tipo, Conjunto, Protecao};

fn mibs(conjunto: Conjunto) -> f64 {
    let dado = vec![0x5au8; 1 << 14];
    let registros = 64;
    let mut amostras = Vec::new();
    for _ in 0..5 {
        let mut p = Protecao::de_segredo_com(&[7u8; 32], conjunto);
        let t = Instant::now();
        for _ in 0..registros {
            std::hint::black_box(p.selar(tipo::DADOS, &dado).unwrap());
        }
        let s = t.elapsed().as_secs_f64();
        amostras.push((registros * dado.len()) as f64 / (1024.0 * 1024.0) / s);
    }
    amostras.sort_by(|a, b| a.partial_cmp(b).unwrap());
    amostras[2]
}

fn main() {
    for c in [Conjunto::Chacha20Poly1305Sha256, Conjunto::Aes128GcmSha256] {
        println!(
            "{c:?}: {:.1} MiB/s (mediana de 5, registros de 16 KiB)",
            mibs(c)
        );
    }
}
