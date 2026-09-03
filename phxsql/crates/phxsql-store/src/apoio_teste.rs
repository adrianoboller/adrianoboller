//! Apoio aos testes UNITARIOS (`#[cfg(test)] mod testes` dentro do `src/`).
//!
//! So para testes: `#[cfg(test)]` no `lib.rs` tira isso do binario de producao.
//! Pedido 150 -- a bateria nao limpava o que criava. O padrao velho era um
//! `rm` que nunca rodava (helper devolvia so o `PathBuf`, sem guarda), entao
//! um teste que falhava no meio deixava o diretorio para tras -- e falhar no
//! meio e o normal de um teste de asserção. `DirTemp` apaga no `Drop`, que o
//! Rust roda mesmo durante um panic (unwind), e nao so no fim feliz do corpo.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Diretorio temporario exclusivo, removido no `Drop`.
pub struct DirTemp(pub PathBuf);

impl DirTemp {
    /// `rotulo` identifica o teste/uso no nome do diretorio (depuracao); a
    /// unicidade real vem do PID do processo de teste e de um contador.
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-ut-{}-{rotulo}-{n}", std::process::id()));
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

// Deref para `Path`: converte um helper local `fn dir(...) -> PathBuf` para
// `-> DirTemp` sem tocar cada chamada no corpo dos testes -- `&d`, `d.join`,
// `d.display()` continuam compilando pelo mesmo caminho de sempre.
impl std::ops::Deref for DirTemp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

// `AsRef<Path>`: o Deref acima nao basta para `fn criar(d: impl AsRef<Path>)`
// -- coercao de Deref so vale quando o alvo e' um `&Path` explicito na
// assinatura, nao para satisfazer um bound generico como `impl AsRef<Path>`.
impl AsRef<Path> for DirTemp {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Prova real do pedido 150, nos dois sentidos: um `rm` no fim do corpo
    /// NUNCA rodaria aqui, porque o teste nunca chega ao fim -- e falhar no
    /// meio, nao no fim, e o caso comum de um teste de asserção. Isso so
    /// prova o `Drop` se o teste FALHA de propósito e o diretorio some mesmo
    /// assim; sem o panico de dentro, o teste provaria so o caminho feliz,
    /// que o helper velho ja cobria.
    #[test]
    fn falha_no_meio_do_teste_ainda_assim_limpa_o_diretorio() {
        let guarda = DirTemp::novo("prova-panico");
        let caminho = guarda.0.clone();
        assert!(
            caminho.is_dir(),
            "o guarda tem de criar o diretorio na hora"
        );

        // O `DirTemp` e' movido para dentro do closure: quando o panic
        // desenrola este quadro, o `Drop` dele roda ali, antes de o
        // `catch_unwind` devolver o erro para fora.
        let resultado = std::panic::catch_unwind(move || {
            let _preso_no_escopo_que_vai_falhar = guarda;
            panic!("falha proposital, so para testar que o Drop roda mesmo assim");
        });

        assert!(resultado.is_err(), "o panico tinha de propagar ate aqui");
        assert!(
            !caminho.exists(),
            "o Drop tinha de ter apagado o diretorio durante o desenrolamento do panic -- \
             e nao apagou, o que e exatamente o defeito que o pedido 150 descreve"
        );
    }

    #[test]
    fn caminho_feliz_tambem_limpa() {
        let caminho = {
            let guarda = DirTemp::novo("prova-feliz");
            let c = guarda.0.clone();
            assert!(c.is_dir());
            c
            // `guarda` sai de escopo aqui, no fim do bloco -- Drop normal.
        };
        assert!(!caminho.exists());
    }
}
