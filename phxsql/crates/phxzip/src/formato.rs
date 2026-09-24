//! Pecas do formato 7z comuns a leitura e a escrita: assinatura, os numeros de
//! tamanho variavel, as marcas de propriedade e a tabela de metodos.
//!
//! Lido em `DOC/7zFormat.txt` do 7-Zip 26.03 (ver `docs/7ZIP.md`).

use alloc::vec::Vec;

use crate::erro::{Erro, Resultado};

/// Os seis bytes que abrem todo 7z.
pub const ASSINATURA: [u8; 6] = [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C];
/// Tamanho do cabecalho de assinatura.
pub const CABECALHO_INICIAL: usize = 32;

pub(crate) const K_FIM: u64 = 0x00;
pub(crate) const K_CABECALHO: u64 = 0x01;
pub(crate) const K_PROPRIEDADES_DO_ARQUIVO: u64 = 0x02;
pub(crate) const K_FLUXOS_ADICIONAIS: u64 = 0x03;
pub(crate) const K_FLUXOS_PRINCIPAIS: u64 = 0x04;
pub(crate) const K_ARQUIVOS: u64 = 0x05;
pub(crate) const K_EMPACOTADOS: u64 = 0x06;
pub(crate) const K_DESEMPACOTADOS: u64 = 0x07;
pub(crate) const K_SUBFLUXOS: u64 = 0x08;
pub(crate) const K_TAMANHO: u64 = 0x09;
pub(crate) const K_CRC: u64 = 0x0A;
pub(crate) const K_PASTA: u64 = 0x0B;
pub(crate) const K_TAMANHOS_DOS_CODERS: u64 = 0x0C;
pub(crate) const K_QUANTOS_SUBFLUXOS: u64 = 0x0D;
pub(crate) const K_FLUXO_VAZIO: u64 = 0x0E;
pub(crate) const K_ARQUIVO_VAZIO: u64 = 0x0F;
pub(crate) const K_NOME: u64 = 0x11;
pub(crate) const K_MTIME: u64 = 0x14;
pub(crate) const K_ATRIBUTOS: u64 = 0x15;
pub(crate) const K_CABECALHO_CODIFICADO: u64 = 0x17;

/// Identificadores de metodo que o PhxZip implementa.
pub const COPY: u64 = 0x00;
/// LZMA.
pub const LZMA: u64 = 0x03_01_01;
/// LZMA2.
pub const LZMA2: u64 = 0x21;
/// 7zAES (AES-256 + SHA-256).
pub const AES: u64 = 0x06_F1_07_01;

/// Nome do metodo recusado, para a mensagem dizer o que era. Tabela de
/// `DOC/Methods.txt`. Recusar pelo nome, e nao por «desconhecido», e o que
/// deixa quem recebeu o erro refazer o arquivo com `-m0=lzma2`.
pub fn nome_recusado(id: u64) -> &'static str {
    match id {
        0x03 => "Delta",
        0x04 | 0x03_03_01_03 => "BCJ (x86)",
        0x05 | 0x03_03_02_05 => "PPC",
        0x06 | 0x03_03_04_01 => "IA64",
        0x07 | 0x03_03_05_01 => "ARM",
        0x08 | 0x03_03_07_01 => "ARMT",
        0x09 | 0x03_03_08_05 => "SPARC",
        0x0A => "ARM64",
        0x0B => "RISCV",
        0x03_03_01_1B => "BCJ2",
        0x03_04_01 => "PPMd",
        0x04_01_08 => "Deflate",
        0x04_01_09 => "Deflate64",
        0x04_02_02 => "BZip2",
        0x04_F7_11_01 => "Zstandard",
        0x06_F1_01_01 => "ZipCrypto",
        _ => "desconhecido",
    }
}

/// Leitor de bytes do cabecalho, que nunca le fora da fatia.
pub(crate) struct Bytes<'a> {
    d: &'a [u8],
    pub pos: usize,
}

impl<'a> Bytes<'a> {
    pub fn novo(d: &'a [u8]) -> Bytes<'a> {
        Bytes { d, pos: 0 }
    }

    pub fn byte(&mut self) -> Resultado<u8> {
        let b = *self
            .d
            .get(self.pos)
            .ok_or(Erro::Corrompido("cabecalho acaba no meio"))?;
        self.pos += 1;
        Ok(b)
    }

    pub fn fatia(&mut self, n: usize) -> Resultado<&'a [u8]> {
        let fim = self
            .pos
            .checked_add(n)
            .ok_or(Erro::Corrompido("tamanho no cabecalho"))?;
        let f = self
            .d
            .get(self.pos..fim)
            .ok_or(Erro::Corrompido("cabecalho acaba no meio"))?;
        self.pos = fim;
        Ok(f)
    }

    pub fn u32(&mut self) -> Resultado<u32> {
        let f = self.fatia(4)?;
        Ok(u32::from_le_bytes([f[0], f[1], f[2], f[3]]))
    }

    pub fn u64(&mut self) -> Resultado<u64> {
        let f = self.fatia(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(f);
        Ok(u64::from_le_bytes(a))
    }

    /// NUMBER do 7z: o numero de bits 1 no alto do primeiro byte diz quantos
    /// bytes seguem, em little-endian.
    pub fn numero(&mut self) -> Resultado<u64> {
        let primeiro = self.byte()?;
        let mut mascara = 0x80u8;
        let mut v = 0u64;
        for i in 0..8 {
            if primeiro & mascara == 0 {
                let alto = (primeiro & mascara.wrapping_sub(1)) as u64;
                return Ok(v | (alto << (8 * i)));
            }
            v |= (self.byte()? as u64) << (8 * i);
            mascara >>= 1;
        }
        Ok(v)
    }

    /// Numero que vira contagem ou tamanho em memoria.
    pub fn quantos(&mut self, teto: usize) -> Resultado<usize> {
        let v = self.numero()?;
        if v > teto as u64 {
            return Err(Erro::Teto("contagem no cabecalho acima do teto"));
        }
        Ok(v as usize)
    }

    /// Vetor de bits, do bit mais alto para o mais baixo de cada byte.
    pub fn bits(&mut self, n: usize) -> Resultado<Vec<bool>> {
        let mut v = Vec::with_capacity(n);
        let mut b = 0u8;
        for i in 0..n {
            if i % 8 == 0 {
                b = self.byte()?;
            }
            v.push(b & (0x80 >> (i % 8)) != 0);
        }
        Ok(v)
    }

    /// «Todos definidos» seguido, se nao, do vetor de bits.
    pub fn bits_ou_todos(&mut self, n: usize) -> Resultado<Vec<bool>> {
        if self.byte()? != 0 {
            Ok(alloc::vec![true; n])
        } else {
            self.bits(n)
        }
    }
}

/// Escritor dos mesmos numeros.
pub(crate) fn gravar_numero(s: &mut Vec<u8>, v: u64) {
    // i bytes seguem; o primeiro carrega 7 - i bits.
    let mut i = 0usize;
    while i < 8 && v >= 1u64 << (7 * (i + 1)) {
        i += 1;
    }
    if i == 8 {
        s.push(0xFF);
        s.extend_from_slice(&v.to_le_bytes());
        return;
    }
    let prefixo = !(0xFFu8 >> i);
    s.push(prefixo | (v >> (8 * i)) as u8);
    for k in 0..i {
        s.push((v >> (8 * k)) as u8);
    }
}

pub(crate) fn gravar_bits(s: &mut Vec<u8>, v: &[bool]) {
    for pedaco in v.chunks(8) {
        let mut b = 0u8;
        for (i, x) in pedaco.iter().enumerate() {
            if *x {
                b |= 0x80 >> i;
            }
        }
        s.push(b);
    }
}

/// FILETIME (100 ns desde 1601) a partir de segundos Unix.
pub fn filetime_de_unix(segundos: i64) -> u64 {
    ((segundos + 11_644_473_600).max(0) as u64).saturating_mul(10_000_000)
}

/// Segundos Unix a partir de FILETIME.
pub fn unix_de_filetime(ft: u64) -> i64 {
    (ft / 10_000_000) as i64 - 11_644_473_600
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numero_ida_e_volta_nas_fronteiras() {
        for v in [
            0u64,
            1,
            0x7F,
            0x80,
            0x3FFF,
            0x4000,
            0x1F_FFFF,
            0x20_0000,
            u32::MAX as u64,
            1 << 56,
            (1 << 56) - 1,
            u64::MAX,
        ] {
            let mut s = Vec::new();
            gravar_numero(&mut s, v);
            let mut b = Bytes::novo(&s);
            assert_eq!(b.numero().unwrap(), v, "{v:#x}");
            assert_eq!(b.pos, s.len(), "{v:#x} sobrou");
        }
        // Exemplo do 7zFormat.txt: 0x80 vira 80 80.
        let mut s = Vec::new();
        gravar_numero(&mut s, 0x80);
        assert_eq!(s, [0x80, 0x80]);
    }

    #[test]
    fn filetime_do_marco_unix() {
        assert_eq!(filetime_de_unix(0), 116_444_736_000_000_000);
        assert_eq!(
            unix_de_filetime(filetime_de_unix(1_790_000_000)),
            1_790_000_000
        );
    }
}
