//! Texto atravessando a fronteira C: ler o que o aplicativo mandou e escrever
//! em buffer de tamanho fixo sem jamais passar do fim.
//!
//! A decisao de quanto copiar e uma funcao PURA (`fatiar`), separada do
//! `unsafe` que toca o ponteiro. E de proposito: o truncamento e a parte do
//! driver que mais quebra aplicativo quando sai errada -- copiar demais
//! estoura o buffer do cliente, copiar de menos sem avisar corrompe o dado em
//! silencio -- e funcao pura se prova com teste de tabela, sem soquete e sem
//! ponteiro.

use crate::tipos::*;

/// O que couber de `dado` num buffer de `cap` bytes com NUL no fim.
///
/// Devolve quantos bytes copiar e se truncou. `cap` <= 0 nao cabe nem o NUL,
/// entao copia zero e conta como truncado -- o chamador decide o erro.
pub fn fatiar(dado: usize, cap: SqlLen) -> (usize, bool) {
    if cap <= 0 {
        return (0, dado > 0);
    }
    let util = (cap as usize) - 1;
    if dado <= util {
        (dado, false)
    } else {
        (util, true)
    }
}

/// Le a string que o aplicativo passou: `len` em bytes ou `SQL_NTS`.
///
/// Bytes invalidos de UTF-8 viram U+FFFD em vez de derrubar a chamada: a
/// string vai para o servidor, e e ele quem recusa o que nao entender --
/// com uma mensagem melhor que "texto invalido" daqui.
///
/// # Safety
///
/// `ptr` precisa apontar para `len` bytes validos (ou uma string com NUL,
/// quando `len == SQL_NTS`).
pub unsafe fn ler_texto(ptr: *const SqlChar, len: SqlInteger) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let bytes = if len == SQL_NTS {
        let mut fim = 0usize;
        while *ptr.add(fim) != 0 {
            fim += 1;
        }
        std::slice::from_raw_parts(ptr, fim)
    } else if len < 0 {
        return String::new();
    } else {
        std::slice::from_raw_parts(ptr, len as usize)
    };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Escreve `dado` como string C no buffer do aplicativo.
///
/// Devolve `(escritos, truncou)`. Buffer nulo escreve nada e serve para o
/// aplicativo que so quer o tamanho (pergunta com NULL, aloca, pergunta de
/// novo -- o vai-e-vem classico do ODBC).
///
/// # Safety
///
/// `buf`, quando nao nulo, precisa ter `cap` bytes de espaco.
pub unsafe fn escrever_texto(dado: &[u8], buf: *mut SqlChar, cap: SqlLen) -> (usize, bool) {
    if buf.is_null() {
        return (0, !dado.is_empty());
    }
    let (n, truncou) = fatiar(dado.len(), cap);
    if cap > 0 {
        std::ptr::copy_nonoverlapping(dado.as_ptr(), buf, n);
        *buf.add(n) = 0;
    }
    (n, truncou)
}

#[cfg(test)]
mod testes {
    use super::*;

    // A tabela cobre as tres bordas que ja quebraram driver alheio: buffer
    // justo (cabe com o NUL), buffer um byte menor (trunca UM byte) e buffer
    // sem espaco nem para o NUL.
    #[test]
    fn fatiar_respeita_o_nul() {
        assert_eq!(fatiar(5, 6), (5, false)); // justo: 5 bytes + NUL
        assert_eq!(fatiar(5, 5), (4, true)); // um a menos: trunca
        assert_eq!(fatiar(5, 1), (0, true)); // so o NUL: copia nada
        assert_eq!(fatiar(0, 1), (0, false)); // vazio cabe em 1
        assert_eq!(fatiar(5, 0), (0, true)); // sem espaco algum
        assert_eq!(fatiar(0, 0), (0, false)); // nada em nada nao trunca
    }

    #[test]
    fn escrever_poe_o_nul_no_fim() {
        let mut buf = [0xAAu8; 8];
        let (n, truncou) = unsafe { escrever_texto(b"abc", buf.as_mut_ptr(), 8) };
        assert_eq!((n, truncou), (3, false));
        assert_eq!(&buf[..4], b"abc\0");
    }

    #[test]
    fn escrever_truncado_avisa_e_termina_em_nul() {
        let mut buf = [0xAAu8; 4];
        let (n, truncou) = unsafe { escrever_texto(b"abcdef", buf.as_mut_ptr(), 4) };
        assert_eq!((n, truncou), (3, true));
        assert_eq!(&buf[..4], b"abc\0");
    }

    #[test]
    fn ler_com_nts_e_com_tamanho() {
        let c = b"ola\0resto";
        assert_eq!(unsafe { ler_texto(c.as_ptr(), SQL_NTS) }, "ola");
        assert_eq!(unsafe { ler_texto(c.as_ptr(), 3) }, "ola");
        assert_eq!(unsafe { ler_texto(std::ptr::null(), SQL_NTS) }, "");
    }
}
