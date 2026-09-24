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
///
/// `Clone` porque o HMAC guarda o estado DEPOIS de absorver o bloco da chave e
/// recomeca dele a cada mensagem -- ver [`ChaveHmac`].
#[derive(Clone)]
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
        // So no binario de teste: e o que deixa a prova do 521 contar o custo
        // por dentro, em vez de medir relogio.
        #[cfg(test)]
        COMPRESSOES.with(|c| c.set(c.get() + 1));
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

/// A chave do HMAC no tamanho do bloco: curta completa com zeros, longa vira
/// o SHA-256 dela (RFC 2104, secao 2).
fn bloco_da_chave(chave: &[u8]) -> [u8; SHA256_BLOCO] {
    let mut chave_bloco = [0u8; SHA256_BLOCO];
    if chave.len() > SHA256_BLOCO {
        chave_bloco[..SHA256_LEN].copy_from_slice(&sha256(chave));
    } else {
        chave_bloco[..chave.len()].copy_from_slice(chave);
    }
    chave_bloco
}

/// Uma chave de HMAC-SHA256 ja preparada: o SHA-256 parado depois de absorver
/// o bloco `K ^ ipad`, e outro depois de `K ^ opad`.
///
/// # Por que existe -- pedido 521
///
/// O PBKDF2 chama o HMAC com a MESMA chave (a senha) 210.000 vezes. O
/// `hmac_sha256` de antes normalizava a chave a cada chamada: senha maior que
/// o bloco era resumida por SHA-256 de novo em toda iteracao, e o custo de um
/// login crescia com o tamanho da senha -- medido em debug, 3,10 s com 8 B,
/// 13,98 s com 1 KiB, e um `CREATE USER` com 1 MiB passou de 300 s. Quem
/// escolhia o tamanho era quem mandava a senha, antes da credencial.
///
/// Preparar a chave uma vez e recomecar dos dois estados e o desenho que a
/// propria RFC 2104 descreve (secao 4, «precompute the intermediate results
/// ... on the blocks K XOR ipad and K XOR opad»), e e o que o OpenSSL faz no
/// PBKDF2 dele (um `HMAC_Init_ex` so, e `HMAC_CTX_copy` por iteracao). A
/// saida e bit a bit a mesma; muda so o custo: a senha longa paga o resumo
/// dela UMA vez, e a iteracao passa de quatro compressoes para duas.
#[derive(Clone)]
pub struct ChaveHmac {
    interno: Sha256,
    externo: Sha256,
}

impl ChaveHmac {
    pub fn nova(chave: &[u8]) -> ChaveHmac {
        let chave_bloco = bloco_da_chave(chave);
        let mut ipad = [0x36u8; SHA256_BLOCO];
        let mut opad = [0x5cu8; SHA256_BLOCO];
        for i in 0..SHA256_BLOCO {
            ipad[i] ^= chave_bloco[i];
            opad[i] ^= chave_bloco[i];
        }
        let mut interno = Sha256::novo();
        interno.atualizar(&ipad);
        let mut externo = Sha256::novo();
        externo.atualizar(&opad);
        ChaveHmac { interno, externo }
    }

    /// O HMAC desta chave sobre `mensagem`.
    pub fn calcular(&self, mensagem: &[u8]) -> [u8; SHA256_LEN] {
        let mut h = self.interno.clone();
        h.atualizar(mensagem);
        let dentro = h.finalizar();

        let mut h = self.externo.clone();
        h.atualizar(&dentro);
        h.finalizar()
    }
}

/// HMAC-SHA256 (RFC 2104).
///
/// Para a mesma chave usada muitas vezes, prepare uma [`ChaveHmac`] e chame
/// `calcular`: esta funcao prepara a chave de novo a cada chamada.
pub fn hmac_sha256(chave: &[u8], mensagem: &[u8]) -> [u8; SHA256_LEN] {
    ChaveHmac::nova(chave).calcular(mensagem)
}

std::thread_local! {
    static ITERACOES_PAGAS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
std::thread_local! {
    static COMPRESSOES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Quantas iteracoes de PBKDF2 esta thread ja pagou, desde que nasceu.
///
/// Existe para a prova do pedido 520 contar POR DENTRO se o login de quem nao
/// existe paga o mesmo PBKDF2 de quem existe, em vez de medir relogio -- que
/// floca, e que no caso de antes teria de separar 1.000 de 210.000 iteracoes
/// pelo tempo. Custa uma soma por bloco de saida (nao por iteracao), numa
/// operacao de dezenas de milissegundos: abaixo de qualquer ruido de medida.
pub fn iteracoes_pagas_nesta_thread() -> u64 {
    ITERACOES_PAGAS.with(std::cell::Cell::get)
}

/// PBKDF2-HMAC-SHA256 (RFC 2898).
///
/// `iteracoes` e o custo: quanto maior, mais caro para quem tenta adivinhar a
/// senha -- e para quem confere. Ver [`crate::senha`] para o valor adotado.
///
/// A chave (a senha) e preparada UMA vez, fora do laco -- ver [`ChaveHmac`]
/// para o motivo e o numero (pedido 521).
pub fn pbkdf2_sha256(senha: &[u8], sal: &[u8], iteracoes: u32, saida: &mut [u8]) {
    let iteracoes = iteracoes.max(1);
    let chave = ChaveHmac::nova(senha);
    let mut bloco = 1u32;
    let mut pos = 0usize;

    while pos < saida.len() {
        ITERACOES_PAGAS.with(|c| c.set(c.get() + u64::from(iteracoes)));
        // U1 = HMAC(senha, sal || INT_BE(bloco))
        let mut entrada = Vec::with_capacity(sal.len() + 4);
        entrada.extend_from_slice(sal);
        entrada.extend_from_slice(&bloco.to_be_bytes());
        let mut u = chave.calcular(&entrada);
        let mut acumulado = u;

        for _ in 1..iteracoes {
            u = chave.calcular(&u);
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

/// Um digito hexadecimal, de qualquer caixa: o valor, ou `None`.
///
/// E o motor dos decodificadores de hexadecimal da casa -- o [`de_hex`], o
/// `%XX` da porta web, o identificador do `uuid`, o `\uXXXX` do JSON. Existe
/// porque o atalho `u8::from_str_radix` nao responde a mesma pergunta: ele
/// aceita um `+` na frente, e so recebe `&str`, o que obriga a FATIAR o texto
/// -- e fatia de `str` por byte entra em panico no meio de um caractere de
/// varios bytes (pedido 446).
pub fn digito_hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Bytes a partir de hexadecimal, ou `None` se o texto nao e so hexadecimal
/// de tamanho par (espaco nas pontas nao conta).
///
/// # Por BYTE, e nao por fatia de texto -- pedido 446
///
/// A versao de antes fatiava `&t[i..i + 2]` e conferia a paridade com
/// `len() % 2`, que conta BYTES. `"a€"` tem quatro bytes, passava na
/// paridade, e o corte do segundo par caia no meio do `€`: panico, e nao
/// `None`. O `de_hex` le texto que vem do fio (a `prova` do pulso do
/// cluster), e a copia dele no `carga::hex_para_bytes` lia o valor `Bin` de
/// todo `inserir` -- DENTRO da trava global de dados, que o panico deixava
/// envenenada para toda conexao seguinte. Lendo os bytes um a um nao ha corte
/// possivel: byte de caractere de varios bytes nunca e digito, e a resposta e
/// `None`.
pub fn de_hex(hex: &str) -> Option<Vec<u8>> {
    let t = hex.trim().as_bytes();
    if t.len() % 2 != 0 {
        return None;
    }
    let mut saida = Vec::with_capacity(t.len() / 2);
    for par in t.chunks_exact(2) {
        saida.push((digito_hex(par[0])? << 4) | digito_hex(par[1])?);
    }
    Some(saida)
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

    // ------------------------------------ PBKDF2 com senha longa (pedido 521)

    /// **Vetor publicado com senha maior que o bloco.** A RFC 6070 e os
    /// vetores usuais de PBKDF2-HMAC-SHA256 param em 25 bytes -- nenhum passa
    /// dos 64 do bloco, que e exatamente onde o 521 mexeu. Estes sao do
    /// Wycheproof (C2SP/wycheproof, `testvectors_v1/pbkdf2_hmacsha256_test.json`,
    /// tcId 52, 53, 54 e 60), conferidos tambem contra o `hashlib.pbkdf2_hmac`
    /// do Python, que roda no OpenSSL: senhas de 65, 129 e 257 bytes, e a de
    /// 65 zeros -- o caso em que a chave longa resumida tem de dar diferente
    /// da mesma chave cortada.
    #[test]
    fn pbkdf2_senha_maior_que_o_bloco_vetores_wycheproof() {
        let casos: [(&[u8], &str, usize, &str); 4] = [
            (
                b"R2IXDgYzZBq69pfzJqNtKwaTZEDIFvvkjbSAqgVnEjkEkEEWPNi86Sbjn7krWd9Mg",
                "d26b99043c8ba3a4",
                32,
                "c8595fa30dc95fb839bebfcc230f06844b2f75a393570b22d6c14d647837b87a",
            ),
            (
                b"crzFm9d0yTcEjdhTWXi8wgNQoTNmHnahoiV1pqa13eTqGy3Iu15KORQc9ILSdgVRzERNkDcr5egjbXJxBerSjtrkkgCAajc5bC5D4pnft86f7TbfcfcpYZ0vsTEMI0RAx",
                "9266da5b8c102b27",
                32,
                "24a86f12235e0232bc80a84635a43934b2d37ae1120b4aa1728a3ead93868980",
            ),
            (
                b"2dzVinoEwDdelebWypX1MoOhYuXZF1rQKz2SZl0uxWcwyo5aAnoBRNzPDv0rQ6bi7B4Z42OPiRXSLhYhDAd2btodv3tMTBD0tKF1nKeYBeeeTzpA1ECAPq9BhzJLUZgsv6uNKfDP35XAMhJHlsjoZykgq0bMPbeUiAymo2CqXkdRGRc8vTAvhNZX8SoVM3pNtYJJrXviu3uGX23sj58Gr0aaJKEv7eyl72Hcnagq4tvnS77bmcvglySm4sppzeg8i",
                "6a06903b78dae6de",
                32,
                "a50be9c16f6bf68808436aa3bc6eec36d3c5653c9c7510c1a4a641755b8325fb",
            ),
            (
                &[0u8; 65],
                "9de9b71eeb9d9a34",
                16,
                "5869f35bb108f1c45605ca8109e6661d",
            ),
        ];
        for (senha, sal, n, esperado) in casos {
            let mut saida = vec![0u8; n];
            pbkdf2_sha256(senha, &de_hex(sal).unwrap(), 4096, &mut saida);
            assert_eq!(para_hex(&saida), esperado, "senha de {} B", senha.len());
        }
    }

    /// O HMAC como ele era ANTES do 521, escrito de novo aqui direto do
    /// SHA-256 de uma vez: a chave normalizada a cada chamada, sem estado
    /// guardado. E a referencia contra a qual a versao preparada se confere
    /// bit a bit -- e nao depende de `ChaveHmac` nem do `Clone` do estado.
    fn hmac_ingenuo(chave: &[u8], mensagem: &[u8]) -> [u8; SHA256_LEN] {
        let mut k = [0u8; SHA256_BLOCO];
        if chave.len() > SHA256_BLOCO {
            k[..SHA256_LEN].copy_from_slice(&sha256(chave));
        } else {
            k[..chave.len()].copy_from_slice(chave);
        }
        let mut dentro: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
        dentro.extend_from_slice(mensagem);
        let mut fora: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
        fora.extend_from_slice(&sha256(&dentro));
        sha256(&fora)
    }

    fn pbkdf2_ingenuo(senha: &[u8], sal: &[u8], iteracoes: u32, saida: &mut [u8]) {
        for (i, pedaco) in saida.chunks_mut(SHA256_LEN).enumerate() {
            let mut entrada = sal.to_vec();
            entrada.extend_from_slice(&(i as u32 + 1).to_be_bytes());
            let mut u = hmac_ingenuo(senha, &entrada);
            let mut acumulado = u;
            for _ in 1..iteracoes.max(1) {
                u = hmac_ingenuo(senha, &u);
                for (a, b) in acumulado.iter_mut().zip(u.iter()) {
                    *a ^= b;
                }
            }
            pedaco.copy_from_slice(&acumulado[..pedaco.len()]);
        }
    }

    /// **A saida nao mudou.** Nao ha vetor publicado para cada tamanho de
    /// senha, entao a versao preparada se confere contra a ingenua em TODO
    /// tamanho de 0 a 300 bytes (atravessa 64, 65, 128, 129, 256, 257), com
    /// uma, duas e tres iteracoes e saida de dois blocos, e nos tamanhos
    /// grandes que o teto do 521 deixa passar.
    #[test]
    fn pbkdf2_confere_bit_a_bit_com_a_versao_ingenua() {
        let sal = b"sal-do-521";
        for tamanho in 0..=300usize {
            let senha: Vec<u8> = (0..tamanho).map(|i| (i * 7 + tamanho) as u8).collect();
            for it in 1..=3u32 {
                let mut a = [0u8; 40];
                let mut b = [0u8; 40];
                pbkdf2_sha256(&senha, sal, it, &mut a);
                pbkdf2_ingenuo(&senha, sal, it, &mut b);
                assert_eq!(a, b, "senha de {tamanho} B, {it} iteracao(oes)");
            }
            assert_eq!(
                hmac_sha256(&senha, b"mensagem"),
                hmac_ingenuo(&senha, b"mensagem"),
                "HMAC com chave de {tamanho} B"
            );
        }
        for tamanho in [1024usize, 4096, 65_535] {
            let senha = vec![0xa5u8; tamanho];
            let mut a = [0u8; 32];
            let mut b = [0u8; 32];
            pbkdf2_sha256(&senha, sal, 2, &mut a);
            pbkdf2_ingenuo(&senha, sal, 2, &mut b);
            assert_eq!(a, b, "senha de {tamanho} B");
        }
    }

    fn compressoes() -> u64 {
        COMPRESSOES.with(std::cell::Cell::get)
    }

    /// **O custo da senha longa e pago UMA vez (pedido 521).** Conta as
    /// compressoes do SHA-256 por dentro, em vez de medir relogio: preparar a
    /// chave custa 2 (os blocos `ipad` e `opad`) mais o resumo da senha quando
    /// ela passa do bloco, e cada iteracao custa 2 -- qualquer que seja o
    /// tamanho da senha.
    ///
    /// Vermelho medido com o laco de antes reposto (`u = hmac_sha256(senha,
    /// &u)`, que prepara a chave a cada volta): a primeira conta que cai e a
    /// senha de 0 B com 10 iteracoes, 40 compressoes contra 22 -- quatro por
    /// iteracao em vez de duas, e mais o resumo da senha longa em CADA volta.
    /// Pela porta de dados, em debug, o login de 1 KiB custava 4,64x o de
    /// 8 B (12.271 ms contra 2.643 ms); depois do conserto, 0,98x.
    #[test]
    fn pbkdf2_prepara_a_chave_uma_vez_so() {
        for tamanho in [0usize, 8, 64, 65, 256, 1024, 65_535] {
            let senha = vec![b'x'; tamanho];
            let resumo_da_chave = if tamanho > SHA256_BLOCO {
                (tamanho as u64 + 9).div_ceil(SHA256_BLOCO as u64)
            } else {
                0
            };
            for it in [1u32, 10, 1_000] {
                let antes = compressoes();
                pbkdf2_sha256(&senha, b"salt", it, &mut [0u8; 32]);
                let gastas = compressoes() - antes;
                assert_eq!(
                    gastas,
                    resumo_da_chave + 2 + 2 * u64::from(it),
                    "senha de {tamanho} B com {it} iteracao(oes)"
                );
            }
        }
    }

    /// A chave preparada e reusavel: duas mensagens pela mesma `ChaveHmac` dao
    /// o mesmo que o HMAC de uma vez -- inclusive o caso 6 da RFC 4231, o de
    /// chave maior que o bloco. Se o `calcular` gastasse o estado guardado em
    /// vez de clona-lo, a segunda chamada sairia errada e so ela acusaria.
    #[test]
    fn chave_hmac_preparada_serve_a_muitas_mensagens() {
        let chave = ChaveHmac::nova(&[0xaa; 131]);
        let m = b"Test Using Larger Than Block-Size Key - Hash Key First";
        for _ in 0..3 {
            assert_eq!(
                para_hex(&chave.calcular(m)),
                "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
            );
        }
        let curta = ChaveHmac::nova(b"Jefe");
        assert_eq!(
            curta.calcular(b"what do ya want for nothing?"),
            hmac_sha256(b"Jefe", b"what do ya want for nothing?")
        );
        assert_eq!(curta.calcular(b"outra"), hmac_ingenuo(b"Jefe", b"outra"));
    }

    /// O contador que a prova do 520 le: soma as iteracoes pedidas, uma vez
    /// por bloco de saida.
    #[test]
    fn o_contador_de_iteracoes_soma_o_que_foi_pago() {
        let antes = iteracoes_pagas_nesta_thread();
        pbkdf2_sha256(b"x", b"y", 17, &mut [0u8; 32]);
        pbkdf2_sha256(b"x", b"y", 5, &mut [0u8; 40]);
        assert_eq!(iteracoes_pagas_nesta_thread() - antes, 17 + 2 * 5);
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
        // Maiuscula e espaco nas pontas continuam valendo: e o que os
        // chamadores de sempre mandam (pino colado de um terminal, vetor de
        // RFC em maiusculas).
        assert_eq!(de_hex("  00FF\n").unwrap(), vec![0, 255]);
    }

    /// **Pedido 446.** Texto que vem do fio, par em BYTES e com um caractere de
    /// varios bytes, nao derruba a thread: e recusado.
    ///
    /// O vermelho medido com o `de_hex` de antes, que fatiava `&t[i..i + 2]`:
    /// `"a€"` tem quatro bytes, passa no `len() % 2`, e o corte `t[2..4]` cai
    /// no meio do `€` -- panico «byte index 2 is not a char boundary». O
    /// `catch_unwind` e para o vermelho dizer o DANO (a thread morreu) em vez
    /// de so abortar o teste.
    #[test]
    fn de_hex_com_caractere_de_varios_bytes_recusa_sem_panico() {
        for torto in ["a€", "a€a€", "€a", "éé", "0é", "a€".repeat(16).as_str()] {
            let r = std::panic::catch_unwind(|| de_hex(torto));
            match r {
                Ok(v) => assert!(v.is_none(), "{torto:?} virou bytes: {v:?}"),
                Err(e) => {
                    let msg = e
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_default();
                    panic!("de_hex({torto:?}) derrubou a thread: {msg}");
                }
            }
        }
    }

    /// **O irmao do 446 dentro da mesma linha.** O `u8::from_str_radix` aceita
    /// um `+` na frente -- `"+f"` e 15 --, entao o `de_hex` de antes lia
    /// `"+f+f"` como `[15, 15]`: dois textos diferentes viravam os mesmos
    /// bytes, e o que devia ser «so hexadecimal» aceitava um sinal.
    #[test]
    fn de_hex_recusa_o_sinal_que_o_from_str_radix_aceita() {
        for torto in ["+f+f", "+0", "0+", "+a+b+c+d"] {
            assert_eq!(
                de_hex(torto),
                None,
                "de_hex aceitou {torto:?}: o sinal passou por digito hexadecimal"
            );
        }
    }

    /// O digito do motor, que o `desescapar` da web e o `uuid` passaram a usar
    /// em vez de copias.
    #[test]
    fn digito_hex_so_aceita_os_dezesseis_de_cada_caixa() {
        let mut aceitos = 0;
        for c in 0u8..=255 {
            if let Some(v) = digito_hex(c) {
                aceitos += 1;
                assert_eq!(u32::from(v), char::from(c).to_digit(16).unwrap());
            }
        }
        assert_eq!(aceitos, 22, "0-9, a-f e A-F");
    }
}
