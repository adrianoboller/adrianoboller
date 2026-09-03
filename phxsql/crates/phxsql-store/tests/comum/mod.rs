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

// Deref para `Path`: o teste que trocar `PathBuf` por `DirTemp` continua
// escrevendo `&d`, `d.join(...)`, `d.display()` sem mexer em cada linha --
// so a criacao do guarda muda. E' o que torna a conversao dos testes
// existentes um edit por arquivo, e nao um por chamada.
impl std::ops::Deref for DirTemp {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

// `AsRef<Path>`: o Deref acima nao basta para `fn criar(d: impl AsRef<Path>)`
// -- coercao de Deref so vale quando o alvo e' um `&Path` explicito na
// assinatura, nao para satisfazer um bound generico. Sem este impl, todo
// `Table::criar(&d, ..)`/`Instancia::nova(&d)` teria de virar `&d.0`.
impl AsRef<std::path::Path> for DirTemp {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

/// Gerador pseudoaleatorio deterministico (xorshift64*), para os testes
/// embaralharem a ordem de insercao sem depender de crates externas.
///
/// `dead_code` fica permitido aqui: cada arquivo em `tests/` e' um BINARIO
/// separado que inclui este modulo inteiro por `mod comum;`, e a maioria usa
/// so o `DirTemp` -- o item que fica sem uso muda de arquivo para arquivo,
/// nao existe um so alvo onde apagar o aviso sem apagar o `Rng` para quem usa.
#[allow(dead_code)]
pub struct Rng(u64);

#[allow(dead_code)]
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
