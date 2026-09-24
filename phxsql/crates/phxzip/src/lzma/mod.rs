//! LZMA e LZMA2, os dois compressores que o PhxZip le e grava.
//!
//! Escritos a partir da especificacao (`DOC/lzma.txt` do 7-Zip, dominio
//! publico) e conferidos contra o que o 7-Zip grava -- os testes do formato
//! abrem arquivos produzidos por ele, e o 7-Zip abre os nossos.
//!
//! O modelo (probabilidades, estado, as quatro distancias repetidas) e um so
//! para os dois lados: o codificador e o decodificador tem de evoluir o MESMO
//! estado bit a bit, e duas copias dele divergiriam no primeiro ajuste.

pub mod dec;
pub mod enc;
mod faixa;
pub mod lzma2;
mod modelo;
mod otimo;

pub use dec::DecodificadorLzma;
#[cfg(feature = "std")]
pub use enc::codificar_bloco_lzma2_em_fio;
pub use enc::{blocos_lzma2, codificar_bloco_lzma2, codificar_lzma2, juntar_blocos_lzma2, Nivel};
pub use lzma2::decodificar_lzma2;

/// Propriedades lc/lp/pb do LZMA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Props {
    /// Bits do byte anterior usados no contexto do literal (0..=8).
    pub lc: u32,
    /// Bits da posicao usados no contexto do literal (0..=4).
    pub lp: u32,
    /// Bits da posicao usados no contexto dos demais bits (0..=4).
    pub pb: u32,
}

impl Props {
    /// O padrao do 7-Zip: lc=3, lp=0, pb=2.
    pub const PADRAO: Props = Props {
        lc: 3,
        lp: 0,
        pb: 2,
    };

    /// Le o byte `(pb*5 + lp)*9 + lc`.
    pub fn do_byte(b: u8) -> Option<Props> {
        let mut d = b as u32;
        if d >= 9 * 5 * 5 {
            return None;
        }
        let lc = d % 9;
        d /= 9;
        let lp = d % 5;
        let pb = d / 5;
        Some(Props { lc, lp, pb })
    }

    /// Grava o byte `(pb*5 + lp)*9 + lc`.
    pub fn para_byte(self) -> u8 {
        ((self.pb * 5 + self.lp) * 9 + self.lc) as u8
    }
}

/// Dicionario a partir do byte de propriedade do LZMA2 (0..=40).
pub fn dicionario_lzma2(p: u8) -> Option<u32> {
    match p {
        40 => Some(u32::MAX),
        0..=39 => Some((2 | (p as u32 & 1)) << (p / 2 + 11)),
        _ => None,
    }
}

/// Menor byte de propriedade do LZMA2 cujo dicionario cobre `tam`.
pub fn byte_lzma2(tam: u32) -> u8 {
    for p in 0..40u8 {
        if dicionario_lzma2(p).unwrap_or(u32::MAX) >= tam {
            return p;
        }
    }
    40
}
