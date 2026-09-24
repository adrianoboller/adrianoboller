//! SHA-1, so para falar o protocolo do MySQL(R).
//!
//! # Por que um algoritmo velho entra aqui
//!
//! O SHA-1 esta quebrado para assinatura desde 2017 e o PhxSql nao o usa em
//! lugar nenhum do proprio formato -- senha continua em PBKDF2-HMAC-SHA256, e
//! integridade continua em CRC-32 e SHA-256. Ele existe por um motivo so: o
//! `mysql_native_password` calcula
//!
//! ```text
//! SHA1(senha) XOR SHA1( sal || SHA1(SHA1(senha)) )
//! ```
//!
//! e nao ha como conversar com um servidor MySQL(R) por esse plugin sem
//! calcular exatamente isso. Escolher outro algoritmo nao e uma opcao: quem
//! define o protocolo e o outro lado.
//!
//! Conferido contra os vetores do FIPS 180-4, como manda a regra do projeto
//! para tudo que e criptografia.
//!
//! # O segundo motivo: TOTP
//!
//! O HMAC-SHA1 entra pelo mesmo caminho -- quem define e o outro lado. Os
//! aplicativos autenticadores (RFC 6238) aceitam SHA-256 no papel, mas o
//! Google Authenticator e parentes ignoram o `algorithm=` do URI e calculam
//! SHA-1; um segredo cadastrado com outro algoritmo gera codigos que nunca
//! batem. HMAC nao depende da resistencia a colisao, que e o que caiu no
//! SHA-1: a RFC 6238 continua segura com ele. Conferido contra a RFC 2202.

const ESTADO_INICIAL: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

/// Tamanho do resumo do SHA-1, em bytes.
pub const SHA1_LEN: usize = 20;
const BLOCO: usize = 64;

pub struct Sha1 {
    estado: [u32; 5],
    buffer: [u8; BLOCO],
    no_buffer: usize,
    total_bits: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Sha1::novo()
    }
}

impl Sha1 {
    pub fn novo() -> Sha1 {
        Sha1 {
            estado: ESTADO_INICIAL,
            buffer: [0; BLOCO],
            no_buffer: 0,
            total_bits: 0,
        }
    }

    pub fn atualizar(&mut self, dados: &[u8]) {
        self.total_bits = self.total_bits.wrapping_add((dados.len() as u64) * 8);
        let mut resto = dados;
        if self.no_buffer > 0 {
            let falta = BLOCO - self.no_buffer;
            let leva = falta.min(resto.len());
            self.buffer[self.no_buffer..self.no_buffer + leva].copy_from_slice(&resto[..leva]);
            self.no_buffer += leva;
            resto = &resto[leva..];
            if self.no_buffer == BLOCO {
                let b = self.buffer;
                self.comprimir(&b);
                self.no_buffer = 0;
            }
        }
        while resto.len() >= BLOCO {
            let (bloco, rest) = resto.split_at(BLOCO);
            let mut b = [0u8; BLOCO];
            b.copy_from_slice(bloco);
            self.comprimir(&b);
            resto = rest;
        }
        if !resto.is_empty() {
            self.buffer[..resto.len()].copy_from_slice(resto);
            self.no_buffer = resto.len();
        }
    }

    pub fn finalizar(mut self) -> [u8; SHA1_LEN] {
        let bits = self.total_bits;
        self.atualizar_sem_contar(&[0x80]);
        while self.no_buffer != 56 {
            self.atualizar_sem_contar(&[0]);
        }
        self.atualizar_sem_contar(&bits.to_be_bytes());
        let mut saida = [0u8; SHA1_LEN];
        for (i, palavra) in self.estado.iter().enumerate() {
            saida[i * 4..i * 4 + 4].copy_from_slice(&palavra.to_be_bytes());
        }
        saida
    }

    /// O enchimento nao entra na contagem de bits -- ela ja foi congelada.
    fn atualizar_sem_contar(&mut self, dados: &[u8]) {
        let guardado = self.total_bits;
        self.atualizar(dados);
        self.total_bits = guardado;
    }

    fn comprimir(&mut self, bloco: &[u8; BLOCO]) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                bloco[i * 4],
                bloco[i * 4 + 1],
                bloco[i * 4 + 2],
                bloco[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.estado;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999u32),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
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
        for (destino, valor) in self.estado.iter_mut().zip([a, b, c, d, e]) {
            *destino = destino.wrapping_add(valor);
        }
    }
}

pub fn sha1(dados: &[u8]) -> [u8; SHA1_LEN] {
    let mut h = Sha1::novo();
    h.atualizar(dados);
    h.finalizar()
}

/// HMAC-SHA1 (RFC 2104), para o TOTP da RFC 6238.
pub fn hmac_sha1(chave: &[u8], mensagem: &[u8]) -> [u8; SHA1_LEN] {
    let mut chave_bloco = [0u8; BLOCO];
    if chave.len() > BLOCO {
        chave_bloco[..SHA1_LEN].copy_from_slice(&sha1(chave));
    } else {
        chave_bloco[..chave.len()].copy_from_slice(chave);
    }
    let mut interno = [0x36u8; BLOCO];
    let mut externo = [0x5cu8; BLOCO];
    for i in 0..BLOCO {
        interno[i] ^= chave_bloco[i];
        externo[i] ^= chave_bloco[i];
    }
    let mut h = Sha1::novo();
    h.atualizar(&interno);
    h.atualizar(mensagem);
    let dentro = h.finalizar();
    let mut h = Sha1::novo();
    h.atualizar(&externo);
    h.atualizar(&dentro);
    h.finalizar()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hash::para_hex;

    /// Os tres vetores do FIPS 180-4 para o SHA-1.
    #[test]
    fn vetores_do_fips_180_4() {
        assert_eq!(
            para_hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            para_hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        // Um milhao de 'a': o vetor que pega erro na contagem de bits.
        let mut h = Sha1::novo();
        for _ in 0..1_000 {
            h.atualizar(&[b'a'; 1_000]);
        }
        assert_eq!(
            para_hex(&h.finalizar()),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn a_cadeia_vazia_tambem_tem_resumo() {
        assert_eq!(
            para_hex(&sha1(b"")),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }

    /// Alimentar de um byte por vez tem de dar o mesmo que de uma vez -- e o
    /// que garante que o buffer de bloco esta certo.
    #[test]
    fn pedaco_a_pedaco_da_o_mesmo() {
        let dados: Vec<u8> = (0u8..=255).cycle().take(1_000).collect();
        let inteiro = sha1(&dados);
        let mut h = Sha1::novo();
        for b in &dados {
            h.atualizar(&[*b]);
        }
        assert_eq!(h.finalizar(), inteiro);
    }

    /// Os sete casos da RFC 2202, secao 3 (HMAC-SHA-1). O 6 e o 7 usam chave
    /// de 80 bytes -- maior que o bloco, o caminho do resumo da chave.
    #[test]
    fn vetores_da_rfc_2202() {
        let casos: [(Vec<u8>, Vec<u8>, &str); 7] = [
            (
                vec![0x0b; 20],
                b"Hi There".to_vec(),
                "b617318655057264e28bc0b6fb378c8ef146be00",
            ),
            (
                b"Jefe".to_vec(),
                b"what do ya want for nothing?".to_vec(),
                "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79",
            ),
            (
                vec![0xaa; 20],
                vec![0xdd; 50],
                "125d7342b9ac11cd91a39af48aa17b4f63f175d3",
            ),
            (
                (1u8..=25).collect(),
                vec![0xcd; 50],
                "4c9007f4026250c6bc8414f9bf50c86c2d7235da",
            ),
            (
                vec![0x0c; 20],
                b"Test With Truncation".to_vec(),
                "4c1a03424b55e07fe7f27be1d58bb9324a9a5a04",
            ),
            (
                vec![0xaa; 80],
                b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
                "aa4ae5e15272d00e95705637ce8a3b55ed402112",
            ),
            (
                vec![0xaa; 80],
                b"Test Using Larger Than Block-Size Key and Larger Than One Block-Size Data"
                    .to_vec(),
                "e8e99d0f45237d786d6bbaa7965c7808bbff1a91",
            ),
        ];
        for (i, (k, m, esperado)) in casos.iter().enumerate() {
            assert_eq!(para_hex(&hmac_sha1(k, m)), *esperado, "caso {}", i + 1);
        }
    }
}
