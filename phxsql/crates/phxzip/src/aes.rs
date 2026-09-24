//! AES-256 (FIPS-197) e o modo CBC (NIST SP 800-38A) -- o que o 7zAES usa.
//!
//! # Por que escrever o AES aqui
//!
//! O 7z cifra com AES-256 em CBC, e o dono decidiu que o PhxZip e o 7-Zip
//! escrito em Rust, sem crate nenhuma. O AES entra do mesmo jeito que o SHA-256
//! do `phxsql-core`: a norma lida, reescrita e provada contra o vetor oficial --
//! FIPS-197 Apendice C.3 para o bloco e SP 800-38A F.2.5/F.2.6 para o CBC.
//!
//! # O que ficou de fora, de proposito
//!
//! * **So a chave de 256 bits.** O 7zAES nao usa outra -- o `Methods.txt` lista
//!   o `06F10701` como «7zAES (AES-256 + SHA-256)». Tamanho que ninguem pede e
//!   codigo que ninguem prova.
//! * **Sem instrucao de CPU (AES-NI).** A crate e portavel por construcao, e o
//!   que decide o custo aqui e a derivacao da chave (2^19 SHA-256), nao o AES:
//!   um `.phz` de configuracao tem poucos KiB.
//! * **Sem tempo constante.** A caixa S e consultada por indice que depende da
//!   chave, e isso vaza por cache para quem roda codigo na mesma maquina. Para o
//!   `.phz` do pedido 450 a senha e PUBLICA (esta no binario e no repositorio),
//!   entao nao ha segredo a proteger desse canal. Quem for usar este AES para
//!   segredo de verdade tem de rever esta linha antes.
//!
//! # De onde veio
//!
//! A norma (FIPS-197 secoes 5.1 a 5.3). O `C/Aes.c` do 7-Zip 26.03 (dominio
//! publico) foi lido para conferir que o CBC do 7z e o CBC da norma --
//! `AesCbc_Encode`/`AesCbc_Decode`, `C/Aes.c:349` e `:367` --, e nada foi
//! colado: la as tabelas sao de 32 bits por coluna; aqui a caixa S sai de uma
//! conta `const` (inverso em GF(2^8) e a transformacao afim), porque digitar 256
//! bytes de memoria e o jeito mais facil de errar um e so descobrir no vetor.

/// Tamanho do bloco do AES, em bytes.
pub const BLOCO: usize = 16;
/// Tamanho da chave do AES-256, em bytes.
pub const CHAVE: usize = 32;
const RODADAS: usize = 14;

/// Multiplicacao por `x` em GF(2^8), modulo x^8 + x^4 + x^3 + x + 1.
const fn xtime(b: u8) -> u8 {
    (b << 1) ^ if b & 0x80 != 0 { 0x1b } else { 0 }
}

/// Multiplicacao em GF(2^8), pelo metodo do campones russo.
const fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            p ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    p
}

/// Inverso multiplicativo em GF(2^8): `a^254`, porque o grupo tem ordem 255.
/// O zero vai para o zero, como a FIPS-197 secao 5.1.1 define.
const fn inverso(a: u8) -> u8 {
    let mut r = 1u8;
    let mut base = a;
    let mut e = 254u8;
    while e > 0 {
        if e & 1 != 0 {
            r = gmul(r, base);
        }
        base = gmul(base, base);
        e >>= 1;
    }
    if a == 0 {
        0
    } else {
        r
    }
}

const fn montar_caixa() -> [u8; 256] {
    let mut s = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let b = inverso(i as u8);
        // A transformacao afim da FIPS-197 (5.1), escrita como rotacoes: e a
        // mesma matriz, sem montar a matriz.
        s[i] = b ^ b.rotate_left(1) ^ b.rotate_left(2) ^ b.rotate_left(3) ^ b.rotate_left(4) ^ 0x63;
        i += 1;
    }
    s
}

const fn inverter_caixa(s: &[u8; 256]) -> [u8; 256] {
    let mut inv = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        inv[s[i] as usize] = i as u8;
        i += 1;
    }
    inv
}

static CAIXA: [u8; 256] = montar_caixa();
static CAIXA_INV: [u8; 256] = inverter_caixa(&montar_caixa());

/// A chave expandida: quinze chaves de rodada de 16 bytes.
pub struct Aes256 {
    rodadas: [[u8; BLOCO]; RODADAS + 1],
}

impl Aes256 {
    /// Expande a chave (FIPS-197 5.2, com Nk = 8).
    pub fn nova(chave: &[u8; CHAVE]) -> Aes256 {
        let mut w = [[0u8; 4]; 4 * (RODADAS + 1)];
        for (i, palavra) in w.iter_mut().take(8).enumerate() {
            palavra.copy_from_slice(&chave[4 * i..4 * i + 4]);
        }
        let mut rcon = 1u8;
        for i in 8..w.len() {
            let mut t = w[i - 1];
            if i % 8 == 0 {
                t.rotate_left(1);
                for b in t.iter_mut() {
                    *b = CAIXA[*b as usize];
                }
                t[0] ^= rcon;
                rcon = xtime(rcon);
            } else if i % 8 == 4 {
                // So o AES-256 tem este passo: a chave de 8 palavras precisa de
                // uma substituicao a mais no meio para nao ficar linear demais.
                for b in t.iter_mut() {
                    *b = CAIXA[*b as usize];
                }
            }
            for j in 0..4 {
                w[i][j] = w[i - 8][j] ^ t[j];
            }
        }
        let mut rodadas = [[0u8; BLOCO]; RODADAS + 1];
        for (r, chave_da_rodada) in rodadas.iter_mut().enumerate() {
            for c in 0..4 {
                chave_da_rodada[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
            }
        }
        Aes256 { rodadas }
    }

    fn somar_chave(&self, b: &mut [u8; BLOCO], r: usize) {
        for (x, k) in b.iter_mut().zip(self.rodadas[r].iter()) {
            *x ^= k;
        }
    }

    /// Cifra um bloco no lugar (FIPS-197 5.1).
    pub fn cifrar_bloco(&self, b: &mut [u8; BLOCO]) {
        self.somar_chave(b, 0);
        for r in 1..=RODADAS {
            for x in b.iter_mut() {
                *x = CAIXA[*x as usize];
            }
            deslocar_linhas(b);
            if r != RODADAS {
                misturar_colunas(b);
            }
            self.somar_chave(b, r);
        }
    }

    /// Decifra um bloco no lugar (FIPS-197 5.3, a cifra inversa direta).
    pub fn decifrar_bloco(&self, b: &mut [u8; BLOCO]) {
        self.somar_chave(b, RODADAS);
        for r in (0..RODADAS).rev() {
            desdeslocar_linhas(b);
            for x in b.iter_mut() {
                *x = CAIXA_INV[*x as usize];
            }
            self.somar_chave(b, r);
            if r != 0 {
                desmisturar_colunas(b);
            }
        }
    }
}

// O estado da FIPS-197 e uma matriz 4x4 guardada por COLUNA: s[l][c] = b[4c + l].
// E a mesma ordem da entrada, e por isso nenhuma copia e preciso para entrar e
// sair dela.

fn deslocar_linhas(b: &mut [u8; BLOCO]) {
    let v = *b;
    for c in 0..4 {
        for l in 1..4 {
            b[4 * c + l] = v[4 * ((c + l) % 4) + l];
        }
    }
}

fn desdeslocar_linhas(b: &mut [u8; BLOCO]) {
    let v = *b;
    for c in 0..4 {
        for l in 1..4 {
            b[4 * ((c + l) % 4) + l] = v[4 * c + l];
        }
    }
}

fn misturar_colunas(b: &mut [u8; BLOCO]) {
    for c in 0..4 {
        let [a0, a1, a2, a3] = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
        b[4 * c] = xtime(a0) ^ xtime(a1) ^ a1 ^ a2 ^ a3;
        b[4 * c + 1] = a0 ^ xtime(a1) ^ xtime(a2) ^ a2 ^ a3;
        b[4 * c + 2] = a0 ^ a1 ^ xtime(a2) ^ xtime(a3) ^ a3;
        b[4 * c + 3] = xtime(a0) ^ a0 ^ a1 ^ a2 ^ xtime(a3);
    }
}

fn desmisturar_colunas(b: &mut [u8; BLOCO]) {
    for c in 0..4 {
        let [a0, a1, a2, a3] = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
        b[4 * c] = gmul(a0, 14) ^ gmul(a1, 11) ^ gmul(a2, 13) ^ gmul(a3, 9);
        b[4 * c + 1] = gmul(a0, 9) ^ gmul(a1, 14) ^ gmul(a2, 11) ^ gmul(a3, 13);
        b[4 * c + 2] = gmul(a0, 13) ^ gmul(a1, 9) ^ gmul(a2, 14) ^ gmul(a3, 11);
        b[4 * c + 3] = gmul(a0, 11) ^ gmul(a1, 13) ^ gmul(a2, 9) ^ gmul(a3, 14);
    }
}

/// Cifra em CBC, no lugar. `None` quando o tamanho nao e multiplo do bloco --
/// o preenchimento e decisao de quem chama (o 7z completa com zeros e guarda o
/// tamanho verdadeiro no cabecalho), e nao deste modo.
pub fn cbc_cifrar(aes: &Aes256, iv: &[u8; BLOCO], dados: &mut [u8]) -> Option<()> {
    if dados.len() % BLOCO != 0 {
        return None;
    }
    let mut anterior = *iv;
    for pedaco in dados.chunks_exact_mut(BLOCO) {
        let mut b = [0u8; BLOCO];
        for (x, (p, a)) in b.iter_mut().zip(pedaco.iter().zip(anterior.iter())) {
            *x = p ^ a;
        }
        aes.cifrar_bloco(&mut b);
        pedaco.copy_from_slice(&b);
        anterior = b;
    }
    Some(())
}

/// Decifra em CBC, no lugar. `None` quando o tamanho nao e multiplo do bloco:
/// num arquivo que veio de fora isso e estrutura quebrada, e quem chama nomeia.
pub fn cbc_decifrar(aes: &Aes256, iv: &[u8; BLOCO], dados: &mut [u8]) -> Option<()> {
    if dados.len() % BLOCO != 0 {
        return None;
    }
    let mut anterior = *iv;
    for pedaco in dados.chunks_exact_mut(BLOCO) {
        let mut b = [0u8; BLOCO];
        b.copy_from_slice(pedaco);
        let cifrado = b;
        aes.decifrar_bloco(&mut b);
        for (x, (d, a)) in pedaco.iter_mut().zip(b.iter().zip(anterior.iter())) {
            *x = d ^ a;
        }
        anterior = cifrado;
    }
    Some(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn hex(s: &str) -> Vec<u8> {
        phxsql_core::hash::de_hex(s).expect("hexadecimal do vetor")
    }

    fn chave(s: &str) -> [u8; CHAVE] {
        hex(s).try_into().expect("chave de 32 bytes")
    }

    /// A caixa S e calculada, nao digitada -- e por isso o teste confere os
    /// pontos que a propria FIPS-197 cita (5.1.1: S(53) = ed) e as pontas.
    #[test]
    fn a_caixa_s_calculada_bate_com_a_norma() {
        assert_eq!(CAIXA[0x00], 0x63);
        assert_eq!(CAIXA[0x01], 0x7c);
        assert_eq!(CAIXA[0x53], 0xed);
        assert_eq!(CAIXA[0xff], 0x16);
        for i in 0..256 {
            assert_eq!(CAIXA_INV[CAIXA[i] as usize] as usize, i);
        }
    }

    /// FIPS-197, Apendice C.3 (AES-256): o vetor oficial de um bloco.
    #[test]
    fn fips_197_apendice_c3() {
        let aes = Aes256::nova(&chave(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        ));
        let mut b: [u8; BLOCO] = hex("00112233445566778899aabbccddeeff").try_into().unwrap();
        aes.cifrar_bloco(&mut b);
        assert_eq!(b.to_vec(), hex("8ea2b7ca516745bfeafc49904b496089"));
        aes.decifrar_bloco(&mut b);
        assert_eq!(b.to_vec(), hex("00112233445566778899aabbccddeeff"));
    }

    const SP_CHAVE: &str = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
    const SP_IV: &str = "000102030405060708090a0b0c0d0e0f";
    const SP_CLARO: &str = "6bc1bee22e409f96e93d7e117393172a\
                            ae2d8a571e03ac9c9eb76fac45af8e51\
                            30c81c46a35ce411e5fbc1191a0a52ef\
                            f69f2445df4f9b17ad2b417be66c3710";
    const SP_CIFRADO: &str = "f58c4c04d6e5f1ba779eabfb5f7bfbd6\
                              9cfc4e967edb808d679f777bc6702c7d\
                              39f23369a9d9bacfa530e26304231461\
                              b2eb05e2c39be9fcda6c19078c6a9d1b";

    /// NIST SP 800-38A, F.2.5 (CBC-AES256.Encrypt).
    #[test]
    fn sp_800_38a_f25_cbc_cifrar() {
        let aes = Aes256::nova(&chave(SP_CHAVE));
        let iv: [u8; BLOCO] = hex(SP_IV).try_into().unwrap();
        let mut dados = hex(SP_CLARO);
        cbc_cifrar(&aes, &iv, &mut dados).unwrap();
        assert_eq!(dados, hex(SP_CIFRADO));
    }

    /// NIST SP 800-38A, F.2.6 (CBC-AES256.Decrypt).
    #[test]
    fn sp_800_38a_f26_cbc_decifrar() {
        let aes = Aes256::nova(&chave(SP_CHAVE));
        let iv: [u8; BLOCO] = hex(SP_IV).try_into().unwrap();
        let mut dados = hex(SP_CIFRADO);
        cbc_decifrar(&aes, &iv, &mut dados).unwrap();
        assert_eq!(dados, hex(SP_CLARO));
    }

    /// Tamanho que nao fecha bloco e recusado, e nao cortado em silencio.
    #[test]
    fn cbc_recusa_o_que_nao_fecha_bloco() {
        let aes = Aes256::nova(&[7u8; CHAVE]);
        let mut dados = vec![0u8; 17];
        assert!(cbc_cifrar(&aes, &[0; BLOCO], &mut dados).is_none());
        assert!(cbc_decifrar(&aes, &[0; BLOCO], &mut dados).is_none());
        assert_eq!(dados, vec![0u8; 17], "mexeu nos dados antes de recusar");
    }
}
