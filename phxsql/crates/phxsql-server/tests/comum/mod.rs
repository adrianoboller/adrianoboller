//! Apoio aos testes de INTEGRACAO do `phxsql-server` (cada arquivo de
//! `tests/` e um binario proprio, e nenhum deles enxerga o
//! `#[cfg(test)] mod apoio_teste` de dentro da lib).
//!
//! Pedido 150 -- a bateria nao limpava o que criava. O padrao velho era um
//! helper que devolvia so o `PathBuf`: sem guarda, o diretorio sobrevivia ao
//! teste, e sobrevivia especialmente quando o teste FALHAVA no meio, que e o
//! caso comum. `DirTemp` apaga no `Drop`, que o Rust roda tambem durante o
//! desenrolamento de um panic.
//!
//! O prefixo `phxsrv-it-` e proprio deste crate: com um prefixo por familia,
//! contar o que sobrou em `/tmp` diz de qual bateria veio o lixo.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Diretorio temporario exclusivo, removido no `Drop`.
///
/// `dead_code` permitido: cada arquivo de `tests/` inclui este modulo inteiro
/// por `mod comum;`, e o item que sobra sem uso muda de arquivo para arquivo.
#[allow(dead_code)]
pub struct DirTemp(pub PathBuf);

#[allow(dead_code)]
impl DirTemp {
    /// `rotulo` identifica o teste no nome do diretorio (so para depuracao);
    /// a unicidade real vem do PID do processo de teste e de um contador.
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("phxsrv-it-{}-{rotulo}-{n}", std::process::id()));
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

// Deref para `Path`: o teste que troca `PathBuf` por `DirTemp` continua
// escrevendo `&d`, `d.join(...)` e `d.display()` sem mexer em cada linha.
impl std::ops::Deref for DirTemp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

// `AsRef<Path>`: o Deref acima nao basta para `fn f(d: impl AsRef<Path>)` --
// a coercao de Deref so vale quando a assinatura pede `&Path` explicito.
impl AsRef<Path> for DirTemp {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}
