//! Apoio aos testes de integracao do PhxZip.
//!
//! O mesmo guarda dos outros crates (`phxsql-store/tests/comum/mod.rs`),
//! escrito aqui porque um teste de integracao nao importa o `tests/` de outro
//! crate -- e trazer o `phxsql-store` como dependencia so para isso quebraria
//! a regra de o PhxZip depender apenas do `phxsql-core`. O conferidor dos
//! temporarios (`phxsql-server/src/conferidor_temporarios.rs`, pedido 150)
//! conta este arquivo como isento, com a quantidade.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Diretorio temporario exclusivo, removido no `Drop` -- inclusive quando o
/// teste falha no meio, que e o caso em que um `remove_dir_all` no fim nunca
/// roda.
pub struct DirTemp(pub PathBuf);

impl DirTemp {
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("phxzip-it-{}-{rotulo}-{n}", std::process::id()));
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

impl std::ops::Deref for DirTemp {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}
