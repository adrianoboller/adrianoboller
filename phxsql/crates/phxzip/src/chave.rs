//! 7zAES: a chave vem da senha por SHA-256 iterado, e o dado vai em AES-256-CBC.
//!
//! O algoritmo foi lido no `CPP/7zip/Crypto/7zAes.cpp` do 7-Zip, que e LGPL --
//! so leitura; nada dali foi colado (ver `docs/7ZIP.md` §2). A derivacao:
//!
//! ```text
//! para r de 0 ate 2^ciclos - 1:
//!     sha256.atualizar(sal || senha_utf16le || r como u64 LE)
//! chave = sha256.finalizar()
//! ```
//!
//! O hash e UM so, alimentado 2^ciclos vezes -- nao e hash de hash. O 7-Zip
//! grava ciclos = 19 (524.288 voltas), e esse custo e o que atrasa quem tenta
//! senha por forca bruta.

use alloc::vec::Vec;

use phxhash::hash::Sha256;

use crate::aes::{Aes256, BLOCO, CHAVE};
use crate::erro::{Erro, Resultado};

/// Ciclos que o 7-Zip grava por padrao, e que o PhxZip grava tambem.
pub const CICLOS_PADRAO: u8 = 19;
/// Acima disto a leitura recusa: 2^24 voltas ja passam de segundos, e um
/// arquivo hostil com 2^62 prenderia quem o abre para sempre. O 7-Zip recusa
/// no mesmo ponto.
pub const CICLOS_MAX: u8 = 24;

/// Parametros do coder 7zAES, lidos ou a gravar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamAes {
    /// log2 do numero de voltas do SHA-256.
    pub ciclos: u8,
    /// Sal (0 a 16 bytes; o 7-Zip grava zero).
    pub sal: Vec<u8>,
    /// Vetor inicial do CBC (completado com zero ate 16).
    pub iv: [u8; BLOCO],
    /// Quantos bytes do IV vao gravados.
    pub iv_len: usize,
}

impl ParamAes {
    /// Le as propriedades do coder.
    pub fn ler(p: &[u8]) -> Resultado<ParamAes> {
        let b0 = *p
            .first()
            .ok_or(Erro::Corrompido("propriedades 7zAES vazias"))?;
        let ciclos = b0 & 0x3F;
        let mut r = ParamAes {
            ciclos,
            sal: Vec::new(),
            iv: [0; BLOCO],
            iv_len: 0,
        };
        if b0 & 0xC0 == 0 {
            if p.len() != 1 {
                return Err(Erro::Corrompido("propriedades 7zAES com sobra"));
            }
            return Ok(r);
        }
        let b1 = *p
            .get(1)
            .ok_or(Erro::Corrompido("propriedades 7zAES truncadas"))?;
        let sal = ((b0 >> 7) & 1) as usize + (b1 >> 4) as usize;
        let iv = ((b0 >> 6) & 1) as usize + (b1 & 0x0F) as usize;
        if p.len() != 2 + sal + iv {
            return Err(Erro::Corrompido("tamanho das propriedades 7zAES"));
        }
        r.sal = p[2..2 + sal].to_vec();
        r.iv[..iv].copy_from_slice(&p[2 + sal..]);
        r.iv_len = iv;
        Ok(r)
    }

    /// Grava as propriedades do coder, no mesmo desenho que o 7-Zip grava.
    pub fn gravar(&self) -> Vec<u8> {
        let s = self.sal.len();
        let i = self.iv_len;
        let mut v = Vec::with_capacity(2 + s + i);
        v.push(self.ciclos | if s == 0 { 0 } else { 0x80 } | if i == 0 { 0 } else { 0x40 });
        if s != 0 || i != 0 {
            v.push(((if s == 0 { 0 } else { s - 1 } << 4) | if i == 0 { 0 } else { i - 1 }) as u8);
            v.extend_from_slice(&self.sal);
            v.extend_from_slice(&self.iv[..i]);
        }
        v
    }

    /// Deriva a chave e monta o AES.
    pub fn cifra(&self, senha: &str) -> Resultado<Aes256> {
        Ok(Aes256::novo(&derivar(senha, &self.sal, self.ciclos)?))
    }
}

/// A derivacao do 7zAES. `ciclos = 0x3F` e o caso especial «sem hash» do
/// 7-Zip (sal e senha copiados direto para a chave) -- lido para abrir o que
/// existe, nunca gravado.
pub fn derivar(senha: &str, sal: &[u8], ciclos: u8) -> Resultado<[u8; CHAVE]> {
    let mut s16 = Vec::with_capacity(senha.len() * 2);
    for u in senha.encode_utf16() {
        s16.extend_from_slice(&u.to_le_bytes());
    }
    let mut chave = [0u8; CHAVE];
    if ciclos == 0x3F {
        let junto: Vec<u8> = sal.iter().chain(s16.iter()).copied().take(CHAVE).collect();
        chave[..junto.len()].copy_from_slice(&junto);
        apagar(&mut s16);
        return Ok(chave);
    }
    if ciclos > CICLOS_MAX {
        apagar(&mut s16);
        return Err(Erro::Teto("ciclos do 7zAES acima de 24"));
    }
    let mut h = Sha256::novo();
    for r in 0..(1u64 << ciclos) {
        h.atualizar(sal);
        h.atualizar(&s16);
        h.atualizar(&r.to_le_bytes());
    }
    chave.copy_from_slice(&h.finalizar());
    apagar(&mut s16);
    Ok(chave)
}

fn apagar(v: &mut [u8]) {
    for b in v.iter_mut() {
        // SAFETY: escrita volatil num byte valido; so impede o compilador de
        // tirar o apagamento da senha por «nao ser lida depois».
        unsafe { core::ptr::write_volatile(b, 0) };
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn propriedades_ida_e_volta_no_desenho_do_7zip() {
        let p = ParamAes {
            ciclos: 19,
            sal: Vec::new(),
            iv: [7; 16],
            iv_len: 16,
        };
        let g = p.gravar();
        // 19 | 0x40 (tem IV); segundo byte: sal 0, iv 16-1 = 0x0F.
        assert_eq!(&g[..2], &[0x53, 0x0F]);
        assert_eq!(ParamAes::ler(&g).unwrap(), p);
    }

    #[test]
    fn ciclos_acima_do_teto_recusam_sem_calcular() {
        assert_eq!(
            derivar("x", &[], 40),
            Err(Erro::Teto("ciclos do 7zAES acima de 24"))
        );
    }

    #[test]
    fn um_ciclo_e_o_sha256_da_concatenacao() {
        // 2^0 = 1 volta: sha256(sal || "ab" em UTF-16LE || 0u64).
        let k = derivar("ab", &[1, 2], 0).unwrap();
        let mut entrada = alloc::vec![1u8, 2, b'a', 0, b'b', 0];
        entrada.extend_from_slice(&[0; 8]);
        assert_eq!(k, phxhash::hash::sha256(&entrada));
    }
}
