//! O codificador de faixa (range coder) do LZMA, nos dois sentidos.

use alloc::vec::Vec;

use crate::erro::{Erro, Resultado};

const BITS_DO_MODELO: u32 = 11;
const TOTAL: u32 = 1 << BITS_DO_MODELO;
const MOVER: u32 = 5;
const TOPO: u32 = 1 << 24;

/// Decodificador de faixa sobre uma fatia.
pub(crate) struct Decodificador<'a> {
    dados: &'a [u8],
    pos: usize,
    faixa: u32,
    codigo: u32,
}

impl<'a> Decodificador<'a> {
    pub fn novo(dados: &'a [u8]) -> Resultado<Decodificador<'a>> {
        if dados.len() < 5 {
            return Err(Erro::Corrompido("fluxo LZMA curto demais"));
        }
        if dados[0] != 0 {
            return Err(Erro::Corrompido("primeiro byte do fluxo LZMA nao e zero"));
        }
        let codigo = u32::from_be_bytes([dados[1], dados[2], dados[3], dados[4]]);
        Ok(Decodificador {
            dados,
            pos: 5,
            faixa: 0xFFFF_FFFF,
            codigo,
        })
    }

    /// Bytes consumidos ate aqui.
    pub fn consumidos(&self) -> usize {
        self.pos
    }

    #[inline]
    fn normalizar(&mut self) -> Resultado<()> {
        if self.faixa < TOPO {
            // Passar do fim e dado corrompido, nunca leitura fora da fatia.
            let b = *self
                .dados
                .get(self.pos)
                .ok_or(Erro::Corrompido("fluxo LZMA acabou antes do fim"))?;
            self.pos += 1;
            self.faixa <<= 8;
            self.codigo = (self.codigo << 8) | b as u32;
        }
        Ok(())
    }

    #[inline]
    pub fn bit(&mut self, p: &mut u16) -> Resultado<u32> {
        let limite = (self.faixa >> BITS_DO_MODELO) * (*p as u32);
        let b = if self.codigo < limite {
            self.faixa = limite;
            *p += ((TOTAL - *p as u32) >> MOVER) as u16;
            0
        } else {
            self.faixa -= limite;
            self.codigo -= limite;
            *p -= *p >> MOVER;
            1
        };
        self.normalizar()?;
        Ok(b)
    }

    pub fn diretos(&mut self, n: u32) -> Resultado<u32> {
        let mut r = 0u32;
        for _ in 0..n {
            self.faixa >>= 1;
            let b = if self.codigo >= self.faixa {
                self.codigo -= self.faixa;
                1
            } else {
                0
            };
            r = (r << 1) | b;
            self.normalizar()?;
        }
        Ok(r)
    }

    pub fn arvore(&mut self, probs: &mut [u16], bits: u32) -> Resultado<u32> {
        let mut m = 1usize;
        for _ in 0..bits {
            m = (m << 1) | self.bit(&mut probs[m])? as usize;
        }
        Ok(m as u32 - (1 << bits))
    }

    pub fn arvore_reversa(&mut self, probs: &mut [u16], bits: u32) -> Resultado<u32> {
        let mut m = 1usize;
        let mut s = 0u32;
        for i in 0..bits {
            let b = self.bit(&mut probs[m])?;
            m = (m << 1) | b as usize;
            s |= b << i;
        }
        Ok(s)
    }

    /// Ao fim do fluxo, o codigo tem de ter voltado a zero: e a conferencia
    /// que o proprio 7-Zip faz, e a unica que o LZMA sem marca de fim oferece.
    pub fn terminou_limpo(&self) -> bool {
        self.codigo == 0
    }
}

/// Codificador de faixa que escreve num `Vec`.
pub(crate) struct Codificador {
    pub saida: Vec<u8>,
    baixo: u64,
    faixa: u32,
    cache: u8,
    pendentes: u64,
}

impl Codificador {
    pub fn novo() -> Codificador {
        Codificador {
            saida: Vec::new(),
            baixo: 0,
            faixa: 0xFFFF_FFFF,
            cache: 0,
            pendentes: 1,
        }
    }

    /// Teto do que `terminar` vai escrever -- e o que o LZMA2 confere para
    /// fechar o pedaco antes dos 64 KiB comprimidos.
    pub fn tamanho_ao_terminar(&self) -> usize {
        self.saida.len() + self.pendentes as usize + 4
    }

    fn deslocar(&mut self) {
        if (self.baixo as u32) < 0xFF00_0000 || (self.baixo >> 32) != 0 {
            let vai_um = (self.baixo >> 32) as u8;
            let mut t = self.cache;
            loop {
                self.saida.push(t.wrapping_add(vai_um));
                t = 0xFF;
                self.pendentes -= 1;
                if self.pendentes == 0 {
                    break;
                }
            }
            self.cache = (self.baixo >> 24) as u8;
        }
        self.pendentes += 1;
        self.baixo = (self.baixo & 0x00FF_FFFF) << 8;
    }

    #[inline]
    pub fn bit(&mut self, p: &mut u16, b: u32) {
        let limite = (self.faixa >> BITS_DO_MODELO) * (*p as u32);
        if b == 0 {
            self.faixa = limite;
            *p += ((TOTAL - *p as u32) >> MOVER) as u16;
        } else {
            self.baixo += limite as u64;
            self.faixa -= limite;
            *p -= *p >> MOVER;
        }
        while self.faixa < TOPO {
            self.faixa <<= 8;
            self.deslocar();
        }
    }

    pub fn diretos(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.faixa >>= 1;
            if (v >> i) & 1 != 0 {
                self.baixo += self.faixa as u64;
            }
            while self.faixa < TOPO {
                self.faixa <<= 8;
                self.deslocar();
            }
        }
    }

    pub fn arvore(&mut self, probs: &mut [u16], bits: u32, v: u32) {
        let mut m = 1usize;
        for i in (0..bits).rev() {
            let b = (v >> i) & 1;
            self.bit(&mut probs[m], b);
            m = (m << 1) | b as usize;
        }
    }

    pub fn arvore_reversa(&mut self, probs: &mut [u16], bits: u32, mut v: u32) {
        let mut m = 1usize;
        for _ in 0..bits {
            let b = v & 1;
            v >>= 1;
            self.bit(&mut probs[m], b);
            m = (m << 1) | b as usize;
        }
    }

    pub fn terminar(mut self) -> Vec<u8> {
        for _ in 0..5 {
            self.deslocar();
        }
        self.saida
    }
}
