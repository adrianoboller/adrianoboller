//! Apoio aos testes de integracao.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Diretorio temporario exclusivo, removido no `Drop`.
pub struct DirTemp(pub PathBuf);

impl DirTemp {
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-it-{}-{rotulo}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        DirTemp(p)
    }
}

impl Drop for DirTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Gerador pseudoaleatorio deterministico (xorshift64*), para os testes
/// embaralharem a ordem de insercao sem depender de crates externas.
pub struct Rng(u64);

impl Rng {
    pub fn nova(semente: u64) -> Rng {
        Rng(semente | 1)
    }

    pub fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Embaralha um vetor (Fisher-Yates).
    pub fn embaralhar<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.proximo() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}
