//! As assinaturas que os servicos poem no webhook: HMAC-SHA256 (Meta, LINE, Viber, Slack,
//! webhook generico) e HMAC-SHA1 (Twilio). Escritas aqui sobre o `sha2` que o agente ja
//! usa, em vez de puxar crate nova para quinze linhas; o SHA-1 so existe para o Twilio, que
//! ainda assina com ele. Conferidas contra os vetores da RFC 4231, RFC 2202 e FIPS 180.

use base64::Engine;
use sha2::{Digest, Sha256};

const BLOCO: usize = 64;

fn hmac(chave: &[u8], msg: &[u8], hash: fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
    let mut k = if chave.len() > BLOCO {
        hash(chave)
    } else {
        chave.to_vec()
    };
    k.resize(BLOCO, 0);
    let ipad: Vec<u8> = k
        .iter()
        .map(|b| b ^ 0x36)
        .chain(msg.iter().copied())
        .collect();
    let interno = hash(&ipad);
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).chain(interno).collect();
    hash(&opad)
}

pub fn sha256(dados: &[u8]) -> Vec<u8> {
    Sha256::digest(dados).to_vec()
}

pub fn hmac_sha256(chave: &[u8], msg: &[u8]) -> Vec<u8> {
    hmac(chave, msg, sha256)
}

pub fn hmac_sha1(chave: &[u8], msg: &[u8]) -> Vec<u8> {
    hmac(chave, msg, sha1)
}

/// SHA-1 (FIPS 180-4 §6.1). Nao serve para nada novo: so confere a assinatura do Twilio.
pub fn sha1(dados: &[u8]) -> Vec<u8> {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut m = dados.to_vec();
    let bits = (dados.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for bloco in m.chunks(64) {
        let mut w = [0u32; 80];
        for (i, p) in bloco.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().flat_map(|x| x.to_be_bytes()).collect()
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn base64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

/// Comparacao que nao para no primeiro byte diferente: o tempo de resposta nao diz quantos
/// bytes da assinatura forjada estavam certos. Compara os SHA-256 dos dois lados, e nao os
/// lados: sair cedo quando o tamanho difere contava a quem chuta a `?chave=` da URL o
/// tamanho do segredo. O resumo tem sempre 32 bytes, entao o laco tem sempre o mesmo
/// tamanho; o custo de resumir o segredo e o mesmo a cada chute, e o do chute e do chute.
pub fn iguais(a: &[u8], b: &[u8]) -> bool {
    let (ha, hb) = (Sha256::digest(a), Sha256::digest(b));
    ha.iter()
        .zip(hb.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_contra_fips_180() {
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn hmac_sha256_contra_rfc_4231() {
        // Caso 2.
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Caso 6: chave maior que o bloco.
        assert_eq!(
            hex(&hmac_sha256(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn hmac_sha1_contra_rfc_2202() {
        assert_eq!(
            hex(&hmac_sha1(b"Jefe", b"what do ya want for nothing?")),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
    }

    #[test]
    fn iguais_confere_tamanho_e_conteudo() {
        assert!(iguais(b"abc", b"abc"));
        assert!(!iguais(b"abc", b"abd"));
        assert!(!iguais(b"abc", b"ab"));
    }
}
