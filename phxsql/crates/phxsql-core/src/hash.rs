//! SHA-256, HMAC-SHA256 e PBKDF2-HMAC-SHA256, sem dependencias externas.
//!
//! Existe para o PhxSql poder guardar senha como HASH no `config.json` sem
//! puxar uma crate de fora. As tres implementacoes sao conferidas contra os
//! vetores publicados -- FIPS 180-4 para o SHA-256, RFC 4231 para o HMAC e os
//! vetores usuais de PBKDF2-HMAC-SHA256 -- nos testes deste modulo.
//!
//! **Escopo.** Isto cobre derivacao de chave de senha. Nao e uma biblioteca de
//! criptografia de proposito geral e nao deve ser usada como tal.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const ESTADO_INICIAL: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Tamanho do resumo do SHA-256, em bytes.
pub const SHA256_LEN: usize = 32;
/// Tamanho do bloco do SHA-256, em bytes.
pub const SHA256_BLOCO: usize = 64;

/// Estado de um SHA-256 em andamento.
pub struct Sha256 {
    estado: [u32; 8],
    buffer: [u8; SHA256_BLOCO],
    no_buffer: usize,
    total_bits: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::novo()
    }
}

impl Sha256 {
    pub fn novo() -> Sha256 {
        Sha256 {
            estado: ESTADO_INICIAL,
            buffer: [0u8; SHA256_BLOCO],
            no_buffer: 0,
            total_bits: 0,
        }
    }

    pub fn atualizar(&mut self, dados: &[u8]) {
        self.total_bits = self.total_bits.wrapping_add((dados.len() as u64) * 8);
        let mut resto = dados;

        if self.no_buffer > 0 {
            let falta = SHA256_BLOCO - self.no_buffer;
            let n = falta.min(resto.len());
            self.buffer[self.no_buffer..self.no_buffer + n].copy_from_slice(&resto[..n]);
            self.no_buffer += n;
            resto = &resto[n..];
            if self.no_buffer == SHA256_BLOCO {
                let bloco = self.buffer;
                self.comprimir(&bloco);
                self.no_buffer = 0;
            }
        }

        let mut pedacos = resto.chunks_exact(SHA256_BLOCO);
        for bloco in &mut pedacos {
            let mut b = [0u8; SHA256_BLOCO];
            b.copy_from_slice(bloco);
            self.comprimir(&b);
        }
        let sobra = pedacos.remainder();
        if !sobra.is_empty() {
            self.buffer[..sobra.len()].copy_from_slice(sobra);
            self.no_buffer = sobra.len();
        }
    }

    pub fn finalizar(mut self) -> [u8; SHA256_LEN] {
        let bits = self.total_bits;
        self.atualizar_sem_contar(&[0x80]);
        while self.no_buffer != 56 {
            self.atualizar_sem_contar(&[0x00]);
        }
        self.atualizar_sem_contar(&bits.to_be_bytes());

        let mut saida = [0u8; SHA256_LEN];
        for (i, palavra) in self.estado.iter().enumerate() {
            saida[i * 4..i * 4 + 4].copy_from_slice(&palavra.to_be_bytes());
        }
        saida
    }

    /// Alimenta o padding sem mexer no contador de bits da mensagem.
    fn atualizar_sem_contar(&mut self, dados: &[u8]) {
        for &b in dados {
            self.buffer[self.no_buffer] = b;
            self.no_buffer += 1;
            if self.no_buffer == SHA256_BLOCO {
                let bloco = self.buffer;
                self.comprimir(&bloco);
                self.no_buffer = 0;
            }
        }
    }

    fn comprimir(&mut self, bloco: &[u8; SHA256_BLOCO]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                bloco[i * 4],
                bloco[i * 4 + 1],
                bloco[i * 4 + 2],
                bloco[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.estado;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (destino, valor) in self
            .estado
            .iter_mut()
            .zip([a, b, c, d, e, f, g, h].into_iter())
        {
            *destino = destino.wrapping_add(valor);
        }
    }
}

/// SHA-256 de um bloco unico.
pub fn sha256(dados: &[u8]) -> [u8; SHA256_LEN] {
    let mut h = Sha256::novo();
    h.atualizar(dados);
    h.finalizar()
}

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(chave: &[u8], mensagem: &[u8]) -> [u8; SHA256_LEN] {
    let mut chave_bloco = [0u8; SHA256_BLOCO];
    if chave.len() > SHA256_BLOCO {
        chave_bloco[..SHA256_LEN].copy_from_slice(&sha256(chave));
    } else {
        chave_bloco[..chave.len()].copy_from_slice(chave);
    }

    let mut interno = [0x36u8; SHA256_BLOCO];
    let mut externo = [0x5cu8; SHA256_BLOCO];
    for i in 0..SHA256_BLOCO {
        interno[i] ^= chave_bloco[i];
        externo[i] ^= chave_bloco[i];
    }

    let mut h = Sha256::novo();
    h.atualizar(&interno);
    h.atualizar(mensagem);
    let dentro = h.finalizar();

    let mut h = Sha256::novo();
    h.atualizar(&externo);
    h.atualizar(&dentro);
    h.finalizar()
}

/// PBKDF2-HMAC-SHA256 (RFC 2898).
///
/// `iteracoes` e o custo: quanto maior, mais caro para quem tenta adivinhar a
/// senha -- e para quem confere. Ver [`crate::senha`] para o valor adotado.
pub fn pbkdf2_sha256(senha: &[u8], sal: &[u8], iteracoes: u32, saida: &mut [u8]) {
    let iteracoes = iteracoes.max(1);
    let mut bloco = 1u32;
    let mut pos = 0usize;

    while pos < saida.len() {
        // U1 = HMAC(senha, sal || INT_BE(bloco))
        let mut entrada = Vec::with_capacity(sal.len() + 4);
        entrada.extend_from_slice(sal);
        entrada.extend_from_slice(&bloco.to_be_bytes());
        let mut u = hmac_sha256(senha, &entrada);
        let mut acumulado = u;

        for _ in 1..iteracoes {
            u = hmac_sha256(senha, &u);
            for (a, b) in acumulado.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }

        let n = (saida.len() - pos).min(SHA256_LEN);
        saida[pos..pos + n].copy_from_slice(&acumulado[..n]);
        pos += n;
        bloco += 1;
    }
}

/// Comparacao em tempo constante, para nao vazar o segredo pelo tempo gasto.
pub fn iguais_em_tempo_constante(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diferenca = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diferenca |= x ^ y;
    }
    diferenca == 0
}

pub fn para_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn de_hex(hex: &str) -> Option<Vec<u8>> {
    let t = hex.trim();
    if t.len() % 2 != 0 {
        return None;
    }
    (0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------- SHA-256 (FIPS 180-4)

    #[test]
    fn sha256_vetores_oficiais() {
        assert_eq!(
            para_hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            para_hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            para_hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            para_hex(&sha256(
                b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"
            )),
            "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1"
        );
    }

    #[test]
    fn sha256_um_milhao_de_letras_a() {
        let mut h = Sha256::novo();
        for _ in 0..1_000 {
            h.atualizar(&[b'a'; 1_000]);
        }
        assert_eq!(
            para_hex(&h.finalizar()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn sha256_alimentado_em_pedacos_da_o_mesmo() {
        let msg = b"cadastroClientes.reg + .ndx + .bin + .memo + .log";
        let inteiro = sha256(msg);
        for corte in [1usize, 7, 31, 32, 33, 63, 64, 65] {
            let mut h = Sha256::novo();
            for pedaco in msg.chunks(corte) {
                h.atualizar(pedaco);
            }
            assert_eq!(h.finalizar(), inteiro, "quebrado de {corte} em {corte}");
        }
    }

    // ------------------------------------------------- HMAC (RFC 4231)

    #[test]
    fn hmac_vetores_rfc4231() {
        // Caso 1
        assert_eq!(
            para_hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // Caso 2
        assert_eq!(
            para_hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Caso 3
        assert_eq!(
            para_hex(&hmac_sha256(&[0xaa; 20], &[0xdd; 50])),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
        // Caso 6 -- chave maior que o bloco, exercita o pre-hash da chave
        assert_eq!(
            para_hex(&hmac_sha256(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    // ------------------------------------------------------- PBKDF2

    #[test]
    fn pbkdf2_vetores_conhecidos() {
        let mut saida = [0u8; 32];

        pbkdf2_sha256(b"password", b"salt", 1, &mut saida);
        assert_eq!(
            para_hex(&saida),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );

        pbkdf2_sha256(b"password", b"salt", 2, &mut saida);
        assert_eq!(
            para_hex(&saida),
            "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
        );

        pbkdf2_sha256(b"password", b"salt", 4096, &mut saida);
        assert_eq!(
            para_hex(&saida),
            "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
        );
    }

    #[test]
    fn pbkdf2_saida_longa_atravessa_varios_blocos() {
        // Vetor de 40 bytes: exercita a concatenacao de dois blocos.
        let mut saida = [0u8; 40];
        pbkdf2_sha256(
            b"passwordPASSWORDpassword",
            b"saltSALTsaltSALTsaltSALTsaltSALTsalt",
            4096,
            &mut saida,
        );
        assert_eq!(
            para_hex(&saida),
            "348c89dbcbd32b2f32d814b8116e84cf2b17347ebc1800181c4e2a1fb8dd53e1c635518c7dac47e9"
        );
    }

    #[test]
    fn pbkdf2_sal_diferente_muda_tudo() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        pbkdf2_sha256(b"mesma-senha", b"sal-a", 100, &mut a);
        pbkdf2_sha256(b"mesma-senha", b"sal-b", 100, &mut b);
        assert_ne!(
            a, b,
            "o sal e o que impede duas senhas iguais darem o mesmo hash"
        );
    }

    // ------------------------------------------------------ utilidades

    #[test]
    fn comparacao_em_tempo_constante() {
        assert!(iguais_em_tempo_constante(b"abc", b"abc"));
        assert!(!iguais_em_tempo_constante(b"abc", b"abd"));
        assert!(!iguais_em_tempo_constante(b"abc", b"ab"));
        assert!(iguais_em_tempo_constante(b"", b""));
    }

    #[test]
    fn hex_vai_e_volta() {
        let b = vec![0u8, 15, 16, 255];
        assert_eq!(para_hex(&b), "000f10ff");
        assert_eq!(de_hex("000f10ff").unwrap(), b);
        assert!(de_hex("0f1").is_none());
        assert!(de_hex("zz").is_none());
    }
}
