//! Texto atravessando a fronteira: UTF-8 com tamanho explicito, nas duas
//! direcoes.
//!
//! # Por que nao `NUL`-terminado
//!
//! Porque dado de cliente tem byte zero. Um `Bin` e binario por definicao; um
//! `Memo` colado de um arquivo pode ter um `\0` no meio. `strlen` nisso trunca
//! EM SILENCIO -- grava metade e nao avisa --, que e a pior classe de defeito
//! que existe: a que nao da erro.
//!
//! Uma convencao so, inclusive para nome de tabela e de coluna, onde o
//! `NUL`-terminado funcionaria. Duas convencoes no mesmo cabecalho e como se
//! erra: o dia em que alguem usar a errada, ninguem percebe.

use crate::erro::{anotar, PHX_ERRO_BUFFER, PHX_ERRO_PONTEIRO, PHX_ERRO_UTF8, PHX_OK};

/// Le um par `(ponteiro, tamanho)` como fatia de bytes.
///
/// Tamanho zero e valido e nao exige ponteiro valido -- e o caso de "sem
/// motivo", "sem schema", "texto vazio".
///
/// # Safety
///
/// `p` tem de apontar para `tam` bytes legiveis, ou `tam` tem de ser zero.
pub unsafe fn bytes<'a>(p: *const u8, tam: usize) -> Option<&'a [u8]> {
    if tam == 0 {
        return Some(&[]);
    }
    if p.is_null() {
        return None;
    }
    Some(std::slice::from_raw_parts(p, tam))
}

/// Le um par `(ponteiro, tamanho)` como `&str`, conferindo o UTF-8.
///
/// # Safety
///
/// Mesmo contrato do [`bytes`].
pub unsafe fn texto<'a>(p: *const u8, tam: usize) -> Result<&'a str, i32> {
    let b = match bytes(p, tam) {
        Some(b) => b,
        None => return Err(anotar(PHX_ERRO_PONTEIRO, "texto nulo com tamanho > 0")),
    };
    std::str::from_utf8(b).map_err(|e| {
        anotar(
            PHX_ERRO_UTF8,
            format!(
                "texto nao e UTF-8 valido: byte invalido em {}",
                e.valid_up_to()
            ),
        )
    })
}

/// Escreve um texto no buffer do chamador.
///
/// Sempre grava o tamanho necessario em `precisa`, mesmo quando nao coube --
/// e assim que o chamador cresce o buffer e repete sem perder nada. Quando
/// sobra um byte, fecha com `\0` por conforto de quem vai `printf`; o tamanho
/// e a verdade, o `\0` e cortesia.
///
/// # Safety
///
/// `destino` tem de apontar para `cap` bytes gravaveis, ou `cap` ser zero;
/// `precisa` tem de ser nulo ou apontar para um `usize`.
pub unsafe fn escrever(destino: *mut u8, cap: usize, precisa: *mut usize, texto: &str) -> i32 {
    let b = texto.as_bytes();
    if !precisa.is_null() {
        std::ptr::write(precisa, b.len());
    }
    if destino.is_null() || cap < b.len() {
        return anotar(
            PHX_ERRO_BUFFER,
            format!("buffer de {cap} bytes nao cabe os {} necessarios", b.len()),
        );
    }
    std::ptr::copy_nonoverlapping(b.as_ptr(), destino, b.len());
    if cap > b.len() {
        std::ptr::write(destino.add(b.len()), 0);
    }
    PHX_OK
}
