//! AES-256 (FIPS-197) e o modo CBC que o 7zAES usa.
//!
//! As tabelas nao sao digitadas: saem de `const fn` na compilacao, pela
//! definicao da norma (inverso em GF(2^8) seguido da transformacao afim). Uma
//! S-box copiada a mao tem 256 lugares para errar um digito; a calculada tem um
//! so, e o vetor da FIPS-197 o pega.
//!
//! Implementacao por byte com tabela de multiplicacao, sem T-tables. Nao e
//! tempo constante contra quem mede cache na mesma maquina -- o 7zAES protege
//! arquivo em repouso, e quem ja roda codigo ao lado do processo tem caminho
//! mais curto ate a senha do que um ataque de cache.

/// Tamanho do bloco do AES.
pub const BLOCO: usize = 16;
/// Tamanho da chave do AES-256.
pub const CHAVE: usize = 32;

const RODADAS: usize = 14;

const fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            p ^= a;
        }
        let alto = a & 0x80;
        a <<= 1;
        if alto != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

const fn inverso(a: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let mut x = 1u16;
    while x < 256 {
        if gmul(a, x as u8) == 1 {
            return x as u8;
        }
        x += 1;
    }
    0
}

const fn gerar_sbox() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let b = inverso(i as u8);
        t[i] = b ^ b.rotate_left(1) ^ b.rotate_left(2) ^ b.rotate_left(3) ^ b.rotate_left(4) ^ 0x63;
        i += 1;
    }
    t
}

const fn gerar_inversa(s: &[u8; 256]) -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[s[i] as usize] = i as u8;
        i += 1;
    }
    t
}

const fn tabela_mul(k: u8) -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = gmul(i as u8, k);
        i += 1;
    }
    t
}

static SBOX: [u8; 256] = gerar_sbox();
static INV_SBOX: [u8; 256] = gerar_inversa(&gerar_sbox());
static M2: [u8; 256] = tabela_mul(2);
static M3: [u8; 256] = tabela_mul(3);
static M9: [u8; 256] = tabela_mul(9);
static M11: [u8; 256] = tabela_mul(11);
static M13: [u8; 256] = tabela_mul(13);
static M14: [u8; 256] = tabela_mul(14);

/// Chave AES-256 expandida.
#[derive(Clone)]
pub struct Aes256 {
    rodadas: [[u8; BLOCO]; RODADAS + 1],
}

impl Drop for Aes256 {
    fn drop(&mut self) {
        // A chave expandida reconstroi a chave: nao fica na memoria liberada.
        for r in self.rodadas.iter_mut() {
            for b in r.iter_mut() {
                // SAFETY: escrita volatil num byte valido e alinhado.
                unsafe { core::ptr::write_volatile(b, 0) };
            }
        }
    }
}

impl Aes256 {
    /// Expande a chave (FIPS-197 §5.2, Nk = 8).
    pub fn novo(chave: &[u8; CHAVE]) -> Aes256 {
        let mut w = [[0u8; 4]; 4 * (RODADAS + 1)];
        for (i, p) in w.iter_mut().take(8).enumerate() {
            p.copy_from_slice(&chave[4 * i..4 * i + 4]);
        }
        let mut rcon = 1u8;
        for i in 8..w.len() {
            let mut t = w[i - 1];
            if i % 8 == 0 {
                t = [
                    SBOX[t[1] as usize] ^ rcon,
                    SBOX[t[2] as usize],
                    SBOX[t[3] as usize],
                    SBOX[t[0] as usize],
                ];
                rcon = M2[rcon as usize];
            } else if i % 8 == 4 {
                t = [
                    SBOX[t[0] as usize],
                    SBOX[t[1] as usize],
                    SBOX[t[2] as usize],
                    SBOX[t[3] as usize],
                ];
            }
            for k in 0..4 {
                w[i][k] = w[i - 8][k] ^ t[k];
            }
        }
        let mut rodadas = [[0u8; BLOCO]; RODADAS + 1];
        for (r, rk) in rodadas.iter_mut().enumerate() {
            for c in 0..4 {
                rk[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
            }
        }
        Aes256 { rodadas }
    }

    /// Cifra um bloco no lugar.
    pub fn cifrar_bloco(&self, b: &mut [u8; BLOCO]) {
        somar(b, &self.rodadas[0]);
        for r in 1..=RODADAS {
            for x in b.iter_mut() {
                *x = SBOX[*x as usize];
            }
            deslocar_linhas(b);
            if r != RODADAS {
                for c in 0..4 {
                    let s = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
                    let m = |i: usize| s[i] as usize;
                    b[4 * c] = M2[m(0)] ^ M3[m(1)] ^ s[2] ^ s[3];
                    b[4 * c + 1] = s[0] ^ M2[m(1)] ^ M3[m(2)] ^ s[3];
                    b[4 * c + 2] = s[0] ^ s[1] ^ M2[m(2)] ^ M3[m(3)];
                    b[4 * c + 3] = M3[m(0)] ^ s[1] ^ s[2] ^ M2[m(3)];
                }
            }
            somar(b, &self.rodadas[r]);
        }
    }

    /// Decifra um bloco no lugar.
    pub fn decifrar_bloco(&self, b: &mut [u8; BLOCO]) {
        somar(b, &self.rodadas[RODADAS]);
        for r in (0..RODADAS).rev() {
            desdeslocar_linhas(b);
            for x in b.iter_mut() {
                *x = INV_SBOX[*x as usize];
            }
            somar(b, &self.rodadas[r]);
            if r != 0 {
                for c in 0..4 {
                    let s = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
                    let m = |i: usize| s[i] as usize;
                    b[4 * c] = M14[m(0)] ^ M11[m(1)] ^ M13[m(2)] ^ M9[m(3)];
                    b[4 * c + 1] = M9[m(0)] ^ M14[m(1)] ^ M11[m(2)] ^ M13[m(3)];
                    b[4 * c + 2] = M13[m(0)] ^ M9[m(1)] ^ M14[m(2)] ^ M11[m(3)];
                    b[4 * c + 3] = M11[m(0)] ^ M13[m(1)] ^ M9[m(2)] ^ M14[m(3)];
                }
            }
        }
    }

    /// CBC: cifra `dados` no lugar. O tamanho tem de ser multiplo de 16 --
    /// o enchimento e de quem chama, porque o 7zAES enche com zero e guarda o
    /// tamanho real fora (e nao PKCS#7).
    pub fn cifrar_cbc(&self, iv: &[u8; BLOCO], dados: &mut [u8]) {
        debug_assert!(dados.len() % BLOCO == 0);
        let mut anterior = *iv;
        for bloco in dados.chunks_exact_mut(BLOCO) {
            let mut b = [0u8; BLOCO];
            b.copy_from_slice(bloco);
            somar(&mut b, &anterior);
            self.cifrar_bloco(&mut b);
            bloco.copy_from_slice(&b);
            anterior = b;
        }
    }

    /// CBC: decifra `dados` no lugar (multiplo de 16; a sobra e ignorada).
    pub fn decifrar_cbc(&self, iv: &[u8; BLOCO], dados: &mut [u8]) {
        let mut anterior = *iv;
        for bloco in dados.chunks_exact_mut(BLOCO) {
            let mut b = [0u8; BLOCO];
            b.copy_from_slice(bloco);
            let cifrado = b;
            self.decifrar_bloco(&mut b);
            somar(&mut b, &anterior);
            bloco.copy_from_slice(&b);
            anterior = cifrado;
        }
    }
}

fn somar(b: &mut [u8; BLOCO], k: &[u8; BLOCO]) {
    for (x, y) in b.iter_mut().zip(k.iter()) {
        *x ^= *y;
    }
}

// O estado e coluna por coluna: o byte da linha r, coluna c, mora em 4c + r.
fn deslocar_linhas(b: &mut [u8; BLOCO]) {
    let s = *b;
    for c in 0..4 {
        for r in 1..4 {
            b[4 * c + r] = s[4 * ((c + r) % 4) + r];
        }
    }
}

fn desdeslocar_linhas(b: &mut [u8; BLOCO]) {
    let s = *b;
    for c in 0..4 {
        for r in 1..4 {
            b[4 * ((c + r) % 4) + r] = s[4 * c + r];
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxhash::hash::de_hex;

    fn h<const N: usize>(s: &str) -> [u8; N] {
        let v = de_hex(s).unwrap();
        let mut a = [0u8; N];
        a.copy_from_slice(&v);
        a
    }

    #[test]
    fn sbox_bate_com_a_norma_nos_pontos_publicados() {
        // FIPS-197 figura 7: S(00)=63, S(53)=ed, S(ff)=16.
        assert_eq!(SBOX[0x00], 0x63);
        assert_eq!(SBOX[0x53], 0xed);
        assert_eq!(SBOX[0xff], 0x16);
        assert_eq!(INV_SBOX[0x63], 0x00);
    }

    /// FIPS-197 apendice C.3 (AES-256).
    #[test]
    fn vetor_fips197_c3() {
        let aes = Aes256::novo(&h(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        ));
        let mut b: [u8; 16] = h("00112233445566778899aabbccddeeff");
        aes.cifrar_bloco(&mut b);
        assert_eq!(b, h::<16>("8ea2b7ca516745bfeafc49904b496089"));
        aes.decifrar_bloco(&mut b);
        assert_eq!(b, h::<16>("00112233445566778899aabbccddeeff"));
    }

    /// NIST SP 800-38A F.2.5 e F.2.6 (CBC-AES256), dois primeiros blocos.
    #[test]
    fn vetor_sp800_38a_cbc() {
        let aes = Aes256::novo(&h(
            "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4",
        ));
        let iv: [u8; 16] = h("000102030405060708090a0b0c0d0e0f");
        let claro =
            de_hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51").unwrap();
        let mut d = claro.clone();
        aes.cifrar_cbc(&iv, &mut d);
        assert_eq!(
            d,
            de_hex("f58c4c04d6e5f1ba779eabfb5f7bfbd69cfc4e967edb808d679f777bc6702c7d").unwrap()
        );
        aes.decifrar_cbc(&iv, &mut d);
        assert_eq!(d, claro);
    }
}
