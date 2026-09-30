//! AES-128 (FIPS 197) e GCM (NIST SP 800-38D), so no sentido que o TLS usa:
//! cifrar blocos para o contador. Pedido 572, T5.
//!
//! # Por que um AES novo, se o `phxzip` ja tem um
//!
//! O do `phxzip` le a S-box numa TABELA indexada pelo byte do estado -- e o
//! estado depende da chave. O tempo de cada acesso depende de o endereco
//! estar no cache, e isso vaza a chave para quem mede o tempo do outro lado
//! (o ataque de Bernstein, 2005). Para um arquivo zip que se abre na mesma
//! maquina isso e aceitavel; para chave de sessao numa porta de rede, nao.
//!
//! # Como este fica em tempo constante
//!
//! Nenhum indice e nenhum desvio dependem de segredo:
//!
//! - a S-box e CALCULADA, nao consultada: o inverso em GF(2^8) sai de
//!   `x^254` por multiplicacoes com mascara, e a transformacao afim por
//!   rotacoes -- os dezesseis bytes do estado de uma vez, em `u128`;
//! - o `ShiftRows` so move posicoes fixas, e o `MixColumns` usa o `xtime` com
//!   mascara em vez de `if`;
//! - a multiplicacao do GHASH percorre os 128 bits de `H` sempre, trocando o
//!   `if` do algoritmo 1 da SP 800-38D por mascaras.
//!
//! O preco e velocidade: e bem mais lento que um AES por tabela ou por
//! AES-NI. Por isso o servidor PREFERE o ChaCha20-Poly1305 quando o cliente
//! oferece os dois (§4.1.1 da RFC 8446 deixa a escolha ao servidor), e o GCM
//! atende o cliente que so fala AES -- que e o que a §9.1 obriga a ter.
//!
//! Isto NAO e auditoria de tempo constante: o compilador poderia, em tese,
//! transformar uma mascara em desvio. A garantia escrita e a do codigo-fonte.

use crate::error::{PhxError, Result};
use crate::hash::iguais_em_tempo_constante;

const UNS: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;

/// Mascara por byte com os bits `lo..8` de cada byte.
const fn mascara_alta(n: u32) -> u128 {
    ((0xffu8 << n) as u128) * UNS
}

/// Mascara por byte com os bits `0..8-n` de cada byte.
const fn mascara_baixa(n: u32) -> u128 {
    ((0xffu8 >> n) as u128) * UNS
}

/// `xtime` nos dezesseis bytes de uma vez: multiplica cada um por `x` em
/// GF(2^8) com o polinomio de reducao `0x11b`.
fn xtime16(a: u128) -> u128 {
    let alto = (a >> 7) & UNS;
    let reduz = alto ^ (alto << 1) ^ (alto << 3) ^ (alto << 4); // 0x1b por byte
    ((a << 1) & mascara_alta(1)) ^ reduz
}

/// Produto em GF(2^8), byte a byte, sem desvio.
fn mul16(mut a: u128, b: u128) -> u128 {
    let mut r = 0u128;
    for i in 0..8 {
        let bit = (b >> i) & UNS;
        r ^= a & bit.wrapping_mul(0xff);
        a = xtime16(a);
    }
    r
}

/// Rotacao de cada byte `n` bits para a esquerda.
fn rotl16(x: u128, n: u32) -> u128 {
    ((x << n) & mascara_alta(n)) | ((x >> (8 - n)) & mascara_baixa(8 - n))
}

/// A S-box nos dezesseis bytes: inverso (`x^254`, e `0` fica `0`) e a
/// transformacao afim da FIPS 197 §5.1.1.
fn sbox16(x: u128) -> u128 {
    let x2 = mul16(x, x);
    let x3 = mul16(x2, x);
    let x6 = mul16(x3, x3);
    let x12 = mul16(x6, x6);
    let x15 = mul16(x12, x3);
    let x30 = mul16(x15, x15);
    let x60 = mul16(x30, x30);
    let x120 = mul16(x60, x60);
    let x240 = mul16(x120, x120);
    let x252 = mul16(x240, x12);
    let inv = mul16(x252, x2);
    inv ^ rotl16(inv, 1) ^ rotl16(inv, 2) ^ rotl16(inv, 3) ^ rotl16(inv, 4) ^ (0x63 * UNS)
}

fn xtime(b: u8) -> u8 {
    (b << 1) ^ (0x1b & 0u8.wrapping_sub(b >> 7))
}

/// AES-128, so o sentido de cifrar.
#[derive(Clone)]
pub struct Aes128 {
    rodadas: [u128; 11],
}

impl std::fmt::Debug for Aes128 {
    // As subchaves sao a chave: nunca aparecem.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Aes128(..)")
    }
}

impl Aes128 {
    /// A expansao de chave da FIPS 197 §5.2, com a `SubWord` pela S-box
    /// calculada.
    pub fn nova(chave: &[u8; 16]) -> Aes128 {
        let mut w = [0u32; 44];
        for (i, p) in chave.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
        }
        let mut rcon: u8 = 1;
        for i in 4..44 {
            let mut t = w[i - 1];
            if i % 4 == 0 {
                // O `if` e sobre o INDICE, que e publico.
                let sub = sbox16(t.rotate_left(8) as u128) as u32;
                t = sub ^ ((rcon as u32) << 24);
                rcon = xtime(rcon);
            }
            w[i] = w[i - 4] ^ t;
        }
        let mut rodadas = [0u128; 11];
        for (r, k) in rodadas.iter_mut().enumerate() {
            *k = ((w[4 * r] as u128) << 96)
                | ((w[4 * r + 1] as u128) << 64)
                | ((w[4 * r + 2] as u128) << 32)
                | (w[4 * r + 3] as u128);
        }
        Aes128 { rodadas }
    }

    pub fn cifrar_bloco(&self, bloco: &[u8; 16]) -> [u8; 16] {
        let mut s = u128::from_be_bytes(*bloco) ^ self.rodadas[0];
        for r in 1..11 {
            s = sbox16(s);
            let mut b = s.to_be_bytes();
            // ShiftRows: o byte (linha l, coluna c) esta em 4c + l e vem de
            // 4((c + l) mod 4) + l.
            let a = b;
            for c in 0..4 {
                for l in 1..4 {
                    b[4 * c + l] = a[4 * ((c + l) % 4) + l];
                }
            }
            if r != 10 {
                for c in 0..4 {
                    let col = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
                    let todos = col[0] ^ col[1] ^ col[2] ^ col[3];
                    for l in 0..4 {
                        b[4 * c + l] = col[l] ^ todos ^ xtime(col[l] ^ col[(l + 1) % 4]);
                    }
                }
            }
            s = u128::from_be_bytes(b) ^ self.rodadas[r];
        }
        s.to_be_bytes()
    }
}

/// Produto em GF(2^128) do GHASH (SP 800-38D, algoritmo 1), com os 128 bits
/// de `x` percorridos sempre e o `if` trocado por mascara.
fn ghash_mul(x: u128, h: u128) -> u128 {
    const R: u128 = 0xe1 << 120;
    let mut z = 0u128;
    let mut v = h;
    for i in 0..128 {
        let bit = (x >> (127 - i)) & 1;
        z ^= v & 0u128.wrapping_sub(bit);
        v = (v >> 1) ^ (R & 0u128.wrapping_sub(v & 1));
    }
    z
}

/// Tamanho da etiqueta do GCM usada no TLS.
pub const TAG_LEN: usize = 16;

/// AES-128-GCM com nonce de 96 bits.
#[derive(Clone)]
pub struct Gcm {
    aes: Aes128,
    h: u128,
}

impl std::fmt::Debug for Gcm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gcm(..)")
    }
}

impl Gcm {
    pub fn nova(chave: &[u8; 16]) -> Gcm {
        let aes = Aes128::nova(chave);
        let h = u128::from_be_bytes(aes.cifrar_bloco(&[0u8; 16]));
        Gcm { aes, h }
    }

    fn contador(nonce: &[u8; 12], n: u32) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[..12].copy_from_slice(nonce);
        b[12..].copy_from_slice(&n.to_be_bytes());
        b
    }

    /// CTR a partir de `inc32(J0)`, isto e, contador 2.
    fn ctr(&self, nonce: &[u8; 12], dados: &mut [u8]) {
        for (i, pedaco) in dados.chunks_mut(16).enumerate() {
            let fluxo = self
                .aes
                .cifrar_bloco(&Self::contador(nonce, 2u32.wrapping_add(i as u32)));
            for (d, f) in pedaco.iter_mut().zip(fluxo) {
                *d ^= f;
            }
        }
    }

    fn etiqueta(&self, nonce: &[u8; 12], aad: &[u8], cifrado: &[u8]) -> [u8; 16] {
        let mut y = 0u128;
        for parte in [aad, cifrado] {
            for bloco in parte.chunks(16) {
                let mut b = [0u8; 16];
                b[..bloco.len()].copy_from_slice(bloco);
                y = ghash_mul(y ^ u128::from_be_bytes(b), self.h);
            }
        }
        let tamanhos = ((aad.len() as u128 * 8) << 64) | (cifrado.len() as u128 * 8);
        y = ghash_mul(y ^ tamanhos, self.h);
        let ej0 = u128::from_be_bytes(self.aes.cifrar_bloco(&Self::contador(nonce, 1)));
        (y ^ ej0).to_be_bytes()
    }

    /// Cifra e autentica: devolve o cifrado e a etiqueta.
    pub fn selar(&self, nonce: &[u8; 12], aad: &[u8], claro: &[u8]) -> (Vec<u8>, [u8; 16]) {
        let mut c = claro.to_vec();
        self.ctr(nonce, &mut c);
        let t = self.etiqueta(nonce, aad, &c);
        (c, t)
    }

    /// Confere a etiqueta ANTES de decifrar; etiqueta errada nao decifra nada.
    pub fn abrir(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        cifrado: &[u8],
        etiqueta: &[u8; 16],
    ) -> Result<Vec<u8>> {
        let esperada = self.etiqueta(nonce, aad, cifrado);
        if !iguais_em_tempo_constante(&esperada, etiqueta) {
            return Err(PhxError::Autorizacao(
                "AES-GCM: etiqueta nao confere -- dado adulterado ou chave errada".into(),
            ));
        }
        let mut c = cifrado.to_vec();
        self.ctr(nonce, &mut c);
        Ok(c)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hash::{de_hex, para_hex};

    fn h16(s: &str) -> [u8; 16] {
        de_hex(s).unwrap().try_into().unwrap()
    }

    /// A S-box calculada bate com a da FIPS 197 (Figura 7) nas entradas que
    /// a norma mais cita -- e com a propriedade que a define: nenhum ponto fixo.
    #[test]
    fn a_sbox_calculada_e_a_da_norma() {
        let s = |b: u8| (sbox16(b as u128) & 0xff) as u8;
        assert_eq!(s(0x00), 0x63);
        assert_eq!(s(0x01), 0x7c);
        assert_eq!(s(0x53), 0xed);
        assert_eq!(s(0xff), 0x16);
        assert_eq!(s(0x9a), 0xb8);
        for b in 0..=255u8 {
            assert_ne!(s(b), b, "ponto fixo em {b:#04x}");
        }
    }

    /// FIPS 197, Apendice C.1 (AES-128).
    #[test]
    fn fips_197_apendice_c1() {
        let aes = Aes128::nova(&h16("000102030405060708090a0b0c0d0e0f"));
        let c = aes.cifrar_bloco(&h16("00112233445566778899aabbccddeeff"));
        assert_eq!(para_hex(&c), "69c4e0d86a7b0430d8cdb78070b4c55a");
    }

    /// FIPS 197, Apendice B (o exemplo passo a passo).
    #[test]
    fn fips_197_apendice_b() {
        let aes = Aes128::nova(&h16("2b7e151628aed2a6abf7158809cf4f3c"));
        let c = aes.cifrar_bloco(&h16("3243f6a8885a308d313198a2e0370734"));
        assert_eq!(para_hex(&c), "3925841d02dc09fbdc118597196a0b32");
    }

    /// Casos 1 e 2 do documento do GCM (McGrew-Viega), chave e nonce zero.
    #[test]
    fn gcm_casos_1_e_2() {
        let g = Gcm::nova(&[0u8; 16]);
        let (c, t) = g.selar(&[0u8; 12], &[], &[]);
        assert!(c.is_empty());
        assert_eq!(para_hex(&t), "58e2fccefa7e3061367f1d57a4e7455a");
        let (c, t) = g.selar(&[0u8; 12], &[], &[0u8; 16]);
        assert_eq!(para_hex(&c), "0388dace60b6a392f328c2b971b2fe78");
        assert_eq!(para_hex(&t), "ab6e47d42cec13bdf53a67b21257bddf");
        assert_eq!(g.abrir(&[0u8; 12], &[], &c, &t).unwrap(), vec![0u8; 16]);
    }

    #[test]
    fn etiqueta_adulterada_nao_decifra() {
        let g = Gcm::nova(&[7u8; 16]);
        let (c, mut t) = g.selar(&[1u8; 12], b"cab", b"conteudo secreto");
        assert!(g.abrir(&[1u8; 12], b"cab", &c, &t).is_ok());
        assert!(
            g.abrir(&[1u8; 12], b"CAB", &c, &t).is_err(),
            "o AAD nao entrou"
        );
        t[15] ^= 1;
        assert!(g.abrir(&[1u8; 12], b"cab", &c, &t).is_err());
    }
}
