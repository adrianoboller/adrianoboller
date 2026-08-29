//! ChaCha20-Poly1305 (RFC 8439), sem dependencias externas.
//!
//! Existe porque o pedido 101 -- cifrar o `.log`, a `.trash` e o `.reason` --
//! estava travado numa frase: "o projeto nao tem cifra de bloco". Tinha
//! SHA-256, HMAC e PBKDF2, e nenhum AES.
//!
//! # Por que ChaCha20-Poly1305, e nao AES-GCM
//!
//! AES em software puro e uma escolha ruim para quem nao pode usar a
//! instrucao do processador: a implementacao portatil honesta usa tabelas, e
//! tabela em cache vaza a chave por tempo. Fugir disso exige bitslicing, que e
//! muito mais codigo para conferir. O ChaCha20 e feito de soma, XOR e
//! rotacao de 32 bits -- **tempo constante por construcao**, sem tabela
//! nenhuma -- e o Poly1305 idem. Sao cerca de 300 linhas conferiveis contra
//! vetor, contra alguns milhares de AES bitsliced.
//!
//! O mesmo argumento e o que fez o TLS 1.3 e o WireGuard escolherem esta
//! suite para maquina sem AES-NI. O PhxSql compila para Windows, Linux e ARM
//! do mesmo jeito, e nao tem como saber onde vai rodar.
//!
//! # Escopo
//!
//! Isto cifra ARQUIVO EM REPOUSO. Nao e uma biblioteca de proposito geral e
//! nao substitui TLS no fio.
//!
//! # Conferido contra o RFC
//!
//! Todo vetor do RFC 8439 que da para exercitar esta nos testes deste modulo:
//! o bloco do ChaCha20 (secao 2.3.2), a cifragem completa (2.4.2), o Poly1305
//! (2.5.2), a geracao da chave de uma vez so (2.6.2) e o AEAD inteiro, com
//! dado associado (2.8.2). Nada aqui foi aceito por "parecer certo".

use crate::error::{PhxError, Result};
use crate::hash::{iguais_em_tempo_constante, pbkdf2_sha256};

/// Tamanho da chave, em bytes.
pub const CHAVE_LEN: usize = 32;
/// Tamanho do nonce, em bytes.
pub const NONCE_LEN: usize = 12;
/// Tamanho da etiqueta de autenticacao, em bytes.
pub const TAG_LEN: usize = 16;
/// Tamanho do bloco de fluxo do ChaCha20, em bytes.
pub const BLOCO_LEN: usize = 64;

/// As quatro palavras constantes do estado: `"expand 32-byte k"` em ASCII.
const CONSTANTE: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

// ---------------------------------------------------------------------------
// ChaCha20
// ---------------------------------------------------------------------------

#[inline(always)]
fn quarto_de_volta(e: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    e[a] = e[a].wrapping_add(e[b]);
    e[d] = (e[d] ^ e[a]).rotate_left(16);
    e[c] = e[c].wrapping_add(e[d]);
    e[b] = (e[b] ^ e[c]).rotate_left(12);
    e[a] = e[a].wrapping_add(e[b]);
    e[d] = (e[d] ^ e[a]).rotate_left(8);
    e[c] = e[c].wrapping_add(e[d]);
    e[b] = (e[b] ^ e[c]).rotate_left(7);
}

fn u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn estado(chave: &[u8; CHAVE_LEN], contador: u32, nonce: &[u8; NONCE_LEN]) -> [u32; 16] {
    let mut e = [0u32; 16];
    e[..4].copy_from_slice(&CONSTANTE);
    for i in 0..8 {
        e[4 + i] = u32_le(&chave[i * 4..]);
    }
    e[12] = contador;
    for i in 0..3 {
        e[13 + i] = u32_le(&nonce[i * 4..]);
    }
    e
}

/// Um bloco de 64 bytes do fluxo do ChaCha20.
///
/// E a funcao da secao 2.3 do RFC: 20 rodadas (10 duplas) sobre o estado, e o
/// estado inicial somado de volta no fim -- e essa soma final que impede
/// inverter a permutacao.
pub fn chacha20_bloco(
    chave: &[u8; CHAVE_LEN],
    contador: u32,
    nonce: &[u8; NONCE_LEN],
) -> [u8; BLOCO_LEN] {
    let inicial = estado(chave, contador, nonce);
    let mut e = inicial;

    for _ in 0..10 {
        // Coluna.
        quarto_de_volta(&mut e, 0, 4, 8, 12);
        quarto_de_volta(&mut e, 1, 5, 9, 13);
        quarto_de_volta(&mut e, 2, 6, 10, 14);
        quarto_de_volta(&mut e, 3, 7, 11, 15);
        // Diagonal.
        quarto_de_volta(&mut e, 0, 5, 10, 15);
        quarto_de_volta(&mut e, 1, 6, 11, 12);
        quarto_de_volta(&mut e, 2, 7, 8, 13);
        quarto_de_volta(&mut e, 3, 4, 9, 14);
    }

    let mut saida = [0u8; BLOCO_LEN];
    for i in 0..16 {
        let palavra = e[i].wrapping_add(inicial[i]);
        saida[i * 4..i * 4 + 4].copy_from_slice(&palavra.to_le_bytes());
    }
    saida
}

/// Cifra (ou decifra -- e a mesma coisa) `dados` no lugar.
///
/// O contador anda de bloco em bloco a partir de `contador_inicial`. Cifrar e
/// decifrar sao a mesma operacao porque o fluxo entra por XOR.
///
/// # Panico
///
/// Nao ha: o contador satura em vez de dar a volta. Dar a volta reusaria o
/// mesmo bloco de fluxo com a mesma chave e o mesmo nonce, que e exatamente a
/// falha que quebra a cifra -- entao o limite corta a mensagem em 256 GiB, que
/// e o teto do RFC, e nunca reaproveita.
pub fn chacha20(
    chave: &[u8; CHAVE_LEN],
    contador_inicial: u32,
    nonce: &[u8; NONCE_LEN],
    dados: &mut [u8],
) {
    let mut contador = contador_inicial;
    for pedaco in dados.chunks_mut(BLOCO_LEN) {
        let fluxo = chacha20_bloco(chave, contador, nonce);
        for (d, f) in pedaco.iter_mut().zip(fluxo.iter()) {
            *d ^= f;
        }
        contador = contador
            .checked_add(1)
            .expect("mensagem maior que o teto de 256 GiB do ChaCha20");
    }
}

// ---------------------------------------------------------------------------
// Poly1305
// ---------------------------------------------------------------------------

/// Autenticador Poly1305 em andamento.
///
/// A aritmetica e modulo 2^130-5 em cinco membros de 26 bits, que e a forma
/// que cabe em `u32` com produto em `u64` sem precisar de inteiro de 128 bits
/// -- e portanto sem depender do alvo ter `u128` rapido.
pub struct Poly1305 {
    r: [u32; 5],
    h: [u32; 5],
    pad: [u32; 4],
    buffer: [u8; 16],
    no_buffer: usize,
}

impl Poly1305 {
    pub fn novo(chave: &[u8; CHAVE_LEN]) -> Poly1305 {
        let t0 = u32_le(&chave[0..]);
        let t1 = u32_le(&chave[4..]);
        let t2 = u32_le(&chave[8..]);
        let t3 = u32_le(&chave[12..]);

        // O "clamp" da secao 2.5: zera os bits que dariam vazamento de carry.
        Poly1305 {
            r: [
                t0 & 0x3ff_ffff,
                ((t0 >> 26) | (t1 << 6)) & 0x3ff_ff03,
                ((t1 >> 20) | (t2 << 12)) & 0x3ff_c0ff,
                ((t2 >> 14) | (t3 << 18)) & 0x3f0_3fff,
                (t3 >> 8) & 0x00f_ffff,
            ],
            h: [0; 5],
            pad: [
                u32_le(&chave[16..]),
                u32_le(&chave[20..]),
                u32_le(&chave[24..]),
                u32_le(&chave[28..]),
            ],
            buffer: [0; 16],
            no_buffer: 0,
        }
    }

    fn bloco(&mut self, bloco: &[u8; 16], ultimo_parcial: bool) {
        let hibit = if ultimo_parcial { 0 } else { 1 << 24 };

        let t0 = u32_le(&bloco[0..]);
        let t1 = u32_le(&bloco[4..]);
        let t2 = u32_le(&bloco[8..]);
        let t3 = u32_le(&bloco[12..]);

        let h = &mut self.h;
        h[0] = h[0].wrapping_add(t0 & 0x3ff_ffff);
        h[1] = h[1].wrapping_add(((t0 >> 26) | (t1 << 6)) & 0x3ff_ffff);
        h[2] = h[2].wrapping_add(((t1 >> 20) | (t2 << 12)) & 0x3ff_ffff);
        h[3] = h[3].wrapping_add(((t2 >> 14) | (t3 << 18)) & 0x3ff_ffff);
        h[4] = h[4].wrapping_add((t3 >> 8) | hibit);

        let r = self.r;
        let s = [r[1] * 5, r[2] * 5, r[3] * 5, r[4] * 5];

        let m = |a: u32, b: u32| (a as u64) * (b as u64);
        let d0 = m(h[0], r[0]) + m(h[1], s[3]) + m(h[2], s[2]) + m(h[3], s[1]) + m(h[4], s[0]);
        let d1 = m(h[0], r[1]) + m(h[1], r[0]) + m(h[2], s[3]) + m(h[3], s[2]) + m(h[4], s[1]);
        let d2 = m(h[0], r[2]) + m(h[1], r[1]) + m(h[2], r[0]) + m(h[3], s[3]) + m(h[4], s[2]);
        let d3 = m(h[0], r[3]) + m(h[1], r[2]) + m(h[2], r[1]) + m(h[3], r[0]) + m(h[4], s[3]);
        let d4 = m(h[0], r[4]) + m(h[1], r[3]) + m(h[2], r[2]) + m(h[3], r[1]) + m(h[4], r[0]);

        let mut c = (d0 >> 26) as u32;
        h[0] = (d0 as u32) & 0x3ff_ffff;
        let d1 = d1 + c as u64;
        c = (d1 >> 26) as u32;
        h[1] = (d1 as u32) & 0x3ff_ffff;
        let d2 = d2 + c as u64;
        c = (d2 >> 26) as u32;
        h[2] = (d2 as u32) & 0x3ff_ffff;
        let d3 = d3 + c as u64;
        c = (d3 >> 26) as u32;
        h[3] = (d3 as u32) & 0x3ff_ffff;
        let d4 = d4 + c as u64;
        c = (d4 >> 26) as u32;
        h[4] = (d4 as u32) & 0x3ff_ffff;
        h[0] = h[0].wrapping_add(c.wrapping_mul(5));
        c = h[0] >> 26;
        h[0] &= 0x3ff_ffff;
        h[1] = h[1].wrapping_add(c);
    }

    pub fn atualizar(&mut self, mut dados: &[u8]) {
        if self.no_buffer > 0 {
            let falta = 16 - self.no_buffer;
            let leva = falta.min(dados.len());
            self.buffer[self.no_buffer..self.no_buffer + leva].copy_from_slice(&dados[..leva]);
            self.no_buffer += leva;
            dados = &dados[leva..];
            if self.no_buffer == 16 {
                let b = self.buffer;
                self.bloco(&b, false);
                self.no_buffer = 0;
            }
        }

        while dados.len() >= 16 {
            let mut b = [0u8; 16];
            b.copy_from_slice(&dados[..16]);
            self.bloco(&b, false);
            dados = &dados[16..];
        }

        if !dados.is_empty() {
            self.buffer[..dados.len()].copy_from_slice(dados);
            self.no_buffer = dados.len();
        }
    }

    pub fn finalizar(mut self) -> [u8; TAG_LEN] {
        if self.no_buffer > 0 {
            // O bloco parcial leva o 1 explicito no lugar do `hibit`.
            let n = self.no_buffer;
            self.buffer[n] = 1;
            for b in self.buffer.iter_mut().skip(n + 1) {
                *b = 0;
            }
            let b = self.buffer;
            self.bloco(&b, true);
        }

        let h = &mut self.h;
        let mut c = h[1] >> 26;
        h[1] &= 0x3ff_ffff;
        h[2] += c;
        c = h[2] >> 26;
        h[2] &= 0x3ff_ffff;
        h[3] += c;
        c = h[3] >> 26;
        h[3] &= 0x3ff_ffff;
        h[4] += c;
        c = h[4] >> 26;
        h[4] &= 0x3ff_ffff;
        h[0] += c * 5;
        c = h[0] >> 26;
        h[0] &= 0x3ff_ffff;
        h[1] += c;

        // h + (-p), para decidir sem desviar se ja passou do modulo.
        let mut g = [0u32; 5];
        g[0] = h[0].wrapping_add(5);
        c = g[0] >> 26;
        g[0] &= 0x3ff_ffff;
        for i in 1..4 {
            g[i] = h[i].wrapping_add(c);
            c = g[i] >> 26;
            g[i] &= 0x3ff_ffff;
        }
        g[4] = h[4].wrapping_add(c).wrapping_sub(1 << 26);

        // Escolha sem `if`: mascara toda 1 quando g nao ficou negativo.
        let mascara = (g[4] >> 31).wrapping_sub(1);
        for i in 0..5 {
            h[i] = (h[i] & !mascara) | (g[i] & mascara);
        }

        // Reagrupa os cinco membros de 26 bits em quatro palavras de 32.
        let h0 = h[0] | (h[1] << 26);
        let h1 = (h[1] >> 6) | (h[2] << 20);
        let h2 = (h[2] >> 12) | (h[3] << 14);
        let h3 = (h[3] >> 18) | (h[4] << 8);

        let mut f = h0 as u64 + self.pad[0] as u64;
        let s0 = f as u32;
        f = h1 as u64 + self.pad[1] as u64 + (f >> 32);
        let s1 = f as u32;
        f = h2 as u64 + self.pad[2] as u64 + (f >> 32);
        let s2 = f as u32;
        f = h3 as u64 + self.pad[3] as u64 + (f >> 32);
        let s3 = f as u32;

        let mut tag = [0u8; TAG_LEN];
        tag[0..4].copy_from_slice(&s0.to_le_bytes());
        tag[4..8].copy_from_slice(&s1.to_le_bytes());
        tag[8..12].copy_from_slice(&s2.to_le_bytes());
        tag[12..16].copy_from_slice(&s3.to_le_bytes());
        tag
    }
}

/// Poly1305 de uma mensagem inteira, com a chave de uso unico.
pub fn poly1305(chave: &[u8; CHAVE_LEN], mensagem: &[u8]) -> [u8; TAG_LEN] {
    let mut p = Poly1305::novo(chave);
    p.atualizar(mensagem);
    p.finalizar()
}

/// A chave de uso unico do Poly1305, tirada do bloco 0 do ChaCha20 (secao 2.6).
///
/// # Por que o bloco 0 nao cifra nada
///
/// Se o mesmo bloco de fluxo servisse de chave do autenticador E de mascara do
/// texto, quem conhecesse um pedaco do texto claro leria a chave do
/// autenticador. Por isso o contador do texto comeca em 1.
pub fn chave_poly1305(chave: &[u8; CHAVE_LEN], nonce: &[u8; NONCE_LEN]) -> [u8; CHAVE_LEN] {
    let bloco = chacha20_bloco(chave, 0, nonce);
    let mut k = [0u8; CHAVE_LEN];
    k.copy_from_slice(&bloco[..CHAVE_LEN]);
    k
}

// ---------------------------------------------------------------------------
// AEAD
// ---------------------------------------------------------------------------

fn autenticar(chave_um: &[u8; CHAVE_LEN], aad: &[u8], cifrado: &[u8]) -> [u8; TAG_LEN] {
    let mut p = Poly1305::novo(chave_um);
    let zeros = [0u8; 16];

    p.atualizar(aad);
    let sobra = aad.len() % 16;
    if sobra != 0 {
        p.atualizar(&zeros[..16 - sobra]);
    }

    p.atualizar(cifrado);
    let sobra = cifrado.len() % 16;
    if sobra != 0 {
        p.atualizar(&zeros[..16 - sobra]);
    }

    // Os dois comprimentos no fim sao o que impede mover bytes do dado
    // associado para o texto cifrado sem mudar a etiqueta.
    p.atualizar(&(aad.len() as u64).to_le_bytes());
    p.atualizar(&(cifrado.len() as u64).to_le_bytes());
    p.finalizar()
}

/// Cifra e autentica: devolve o texto cifrado e a etiqueta.
///
/// O `aad` (dado associado) nao vai cifrado, mas entra na etiqueta -- e onde
/// se amarra o cabecalho que precisa ficar legivel, como o numero do evento no
/// `.log`.
///
/// # Nonce
///
/// **Nunca repita o par (chave, nonce).** Repetir revela o XOR dos dois textos
/// claros e permite forjar etiqueta. Ver [`Sequencia`], que existe justamente
/// para isso nao depender de quem chama lembrar.
pub fn selar(
    chave: &[u8; CHAVE_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    claro: &[u8],
) -> (Vec<u8>, [u8; TAG_LEN]) {
    let mut cifrado = claro.to_vec();
    chacha20(chave, 1, nonce, &mut cifrado);
    let tag = autenticar(&chave_poly1305(chave, nonce), aad, &cifrado);
    (cifrado, tag)
}

/// Confere a etiqueta e decifra. Etiqueta errada nao decifra nada.
///
/// # Por que conferir ANTES de decifrar
///
/// Devolver texto decifrado de uma mensagem que nao autenticou entrega ao
/// atacante um oraculo: ele manda lixo modificado e aprende pelo resultado.
/// Aqui a etiqueta errada sai como erro e o texto claro nunca chega a existir.
pub fn abrir(
    chave: &[u8; CHAVE_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    cifrado: &[u8],
    tag: &[u8; TAG_LEN],
) -> Result<Vec<u8>> {
    let esperada = autenticar(&chave_poly1305(chave, nonce), aad, cifrado);
    if !iguais_em_tempo_constante(&esperada, tag) {
        return Err(PhxError::Corrompido(
            "etiqueta de autenticacao nao confere: o dado cifrado foi alterado \
             ou a chave esta errada"
                .into(),
        ));
    }
    let mut claro = cifrado.to_vec();
    chacha20(chave, 1, nonce, &mut claro);
    Ok(claro)
}

// ---------------------------------------------------------------------------
// Chave e nonce, do jeito que os tres arquivos vao usar
// ---------------------------------------------------------------------------

/// Deriva a chave de 32 bytes de uma senha, por PBKDF2-HMAC-SHA256.
///
/// Reaproveita o PBKDF2 que ja estava escrito para o hash de senha do
/// `config.json`. O sal fica no cabecalho do arquivo cifrado, em claro: sal
/// nao e segredo, e o papel dele e impedir que a mesma senha derive a mesma
/// chave em dois arquivos.
pub fn chave_de_senha(senha: &str, sal: &[u8], iteracoes: u32) -> [u8; CHAVE_LEN] {
    let mut chave = [0u8; CHAVE_LEN];
    pbkdf2_sha256(senha.as_bytes(), sal, iteracoes, &mut chave);
    chave
}

/// Gerador de nonce para arquivo *append-only*.
///
/// # Por que um tipo, e nao uma convencao escrita no comentario
///
/// O unico jeito de quebrar esta cifra sem quebrar a matematica e repetir o
/// par (chave, nonce). Num arquivo que so cresce isso e facil de errar: basta
/// alguem reabrir o arquivo e recomecar a contagem do zero. Aqui o nonce sai
/// de um prefixo sorteado UMA vez por arquivo (gravado no cabecalho) mais o
/// numero de ordem do registro, que o arquivo ja tem e que nunca se
/// reaproveita -- a mesma garantia do `.reg`, que nunca reusa slot.
///
/// Repetir o nonce passa entao a exigir reescrever um registro que ja existe,
/// que e coisa que *append-only* nao faz.
#[derive(Debug, Clone, Copy)]
pub struct Sequencia {
    prefixo: [u8; 4],
}

impl Sequencia {
    /// Do prefixo sorteado que esta (ou vai) no cabecalho do arquivo.
    pub fn nova(prefixo: [u8; 4]) -> Sequencia {
        Sequencia { prefixo }
    }

    /// O nonce do registro de numero `ordem`.
    pub fn nonce(&self, ordem: u64) -> [u8; NONCE_LEN] {
        let mut n = [0u8; NONCE_LEN];
        n[..4].copy_from_slice(&self.prefixo);
        n[4..].copy_from_slice(&ordem.to_le_bytes());
        n
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn de_hex(s: &str) -> Vec<u8> {
        let limpo: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        (0..limpo.len() / 2)
            .map(|i| u8::from_str_radix(&limpo[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    fn chave_sequencial(base: u8) -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = base.wrapping_add(i as u8);
        }
        k
    }

    /// RFC 8439, secao 2.3.2.
    #[test]
    fn bloco_do_chacha20_bate_com_o_rfc() {
        let chave = chave_sequencial(0);
        let nonce = de_hex("000000090000004a00000000");
        let mut n = [0u8; 12];
        n.copy_from_slice(&nonce);

        let esperado = de_hex(
            "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c0680304 22aa9ac3d46c4e
             d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e",
        );
        assert_eq!(chacha20_bloco(&chave, 1, &n).to_vec(), esperado);
    }

    /// RFC 8439, secao 2.4.2.
    #[test]
    fn cifragem_do_chacha20_bate_com_o_rfc() {
        let chave = chave_sequencial(0);
        let mut n = [0u8; 12];
        n.copy_from_slice(&de_hex("000000000000004a00000000"));

        let claro = b"Ladies and Gentlemen of the class of '99: If I could \
                      offer you only one tip for the future, sunscreen would be it.";
        let esperado = de_hex(
            "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0b
             f91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d8
             07ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab7793736
             5af90bbf74a35be6b40b8eedf2785e42874d",
        );

        let mut dados = claro.to_vec();
        chacha20(&chave, 1, &n, &mut dados);
        assert_eq!(dados, esperado);

        // E decifrar e a mesma operacao.
        chacha20(&chave, 1, &n, &mut dados);
        assert_eq!(dados, claro.to_vec());
    }

    /// RFC 8439, secao 2.5.2.
    #[test]
    fn poly1305_bate_com_o_rfc() {
        let mut chave = [0u8; 32];
        chave.copy_from_slice(&de_hex(
            "85d6be7857556d337f4452fe42d506a801038 08afb0db2fd4abff6af4149f51b",
        ));
        let mensagem = b"Cryptographic Forum Research Group";
        assert_eq!(
            poly1305(&chave, mensagem).to_vec(),
            de_hex("a8061dc1305136c6c22b8baf0c0127a9")
        );
    }

    /// RFC 8439, secao 2.6.2.
    #[test]
    fn chave_de_uma_vez_so_bate_com_o_rfc() {
        let chave = chave_sequencial(0x80);
        let mut n = [0u8; 12];
        n.copy_from_slice(&de_hex("000000000001020304050607"));
        assert_eq!(
            chave_poly1305(&chave, &n).to_vec(),
            de_hex("8ad5a08b905f81cc815040274ab29471a833b637e3fd0da508dbb8e2fdd1a646")
        );
    }

    /// RFC 8439, secao 2.8.2 -- o AEAD inteiro, com dado associado.
    #[test]
    fn aead_bate_com_o_rfc() {
        let claro = b"Ladies and Gentlemen of the class of '99: If I could \
                      offer you only one tip for the future, sunscreen would be it.";
        let aad = de_hex("50515253c0c1c2c3c4c5c6c7");
        let chave = chave_sequencial(0x80);
        let mut n = [0u8; 12];
        n.copy_from_slice(&de_hex("070000004041424344454647"));

        let esperado = de_hex(
            "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6
             3dbea45e8ca967128 2fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36
             92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc
             3ff4def08e4b7a9de576d26586cec64b6116",
        );
        let tag_esperada = de_hex("1ae10b594f09e26a7e902ecbd0600691");

        let (cifrado, tag) = selar(&chave, &n, &aad, claro);
        assert_eq!(cifrado, esperado, "texto cifrado nao bate com o RFC");
        assert_eq!(tag.to_vec(), tag_esperada, "etiqueta nao bate com o RFC");

        let voltou = abrir(&chave, &n, &aad, &cifrado, &tag).unwrap();
        assert_eq!(voltou, claro.to_vec());
    }

    #[test]
    fn um_bit_trocado_no_texto_nao_abre() {
        let chave = chave_sequencial(7);
        let n = Sequencia::nova([1, 2, 3, 4]).nonce(42);
        let (mut cifrado, tag) = selar(&chave, &n, b"cabecalho", b"o segredo inteiro");
        cifrado[3] ^= 0x01;
        assert!(abrir(&chave, &n, b"cabecalho", &cifrado, &tag).is_err());
    }

    #[test]
    fn dado_associado_trocado_nao_abre() {
        let chave = chave_sequencial(7);
        let n = Sequencia::nova([9, 9, 9, 9]).nonce(1);
        let (cifrado, tag) = selar(&chave, &n, b"evento 1", b"o segredo inteiro");
        // O texto cifrado esta intacto; so o cabecalho em claro mudou.
        assert!(abrir(&chave, &n, b"evento 2", &cifrado, &tag).is_err());
    }

    #[test]
    fn texto_vazio_ainda_autentica() {
        let chave = chave_sequencial(3);
        let n = Sequencia::nova([0; 4]).nonce(0);
        let (cifrado, tag) = selar(&chave, &n, b"", b"");
        assert!(cifrado.is_empty());
        assert_eq!(abrir(&chave, &n, b"", &cifrado, &tag).unwrap(), Vec::new());
    }

    /// Tamanho que nao e multiplo de 64 exercita o ultimo bloco parcial do
    /// fluxo, e o que nao e multiplo de 16 exercita o resto do Poly1305.
    #[test]
    fn tamanhos_irregulares_fecham_a_volta() {
        let chave = chave_sequencial(11);
        for n_bytes in [1usize, 15, 16, 17, 63, 64, 65, 127, 200, 1000] {
            let claro: Vec<u8> = (0..n_bytes).map(|i| (i * 7) as u8).collect();
            let nonce = Sequencia::nova([5, 6, 7, 8]).nonce(n_bytes as u64);
            let (cifrado, tag) = selar(&chave, &nonce, b"aad", &claro);
            assert_eq!(cifrado.len(), n_bytes);
            assert_eq!(
                abrir(&chave, &nonce, b"aad", &cifrado, &tag).unwrap(),
                claro,
                "falhou com {n_bytes} bytes"
            );
        }
    }

    /// Alimentar em pedacos de tamanho qualquer tem de dar a mesma etiqueta
    /// que alimentar de uma vez -- e o que o `.log` vai fazer, evento a evento.
    #[test]
    fn poly1305_em_pedacos_da_o_mesmo() {
        let chave = chave_sequencial(0x40);
        let mensagem: Vec<u8> = (0..250u32).map(|i| (i % 251) as u8).collect();
        let inteiro = poly1305(&chave, &mensagem);

        for corte in [1usize, 7, 15, 16, 17, 64, 100] {
            let mut p = Poly1305::novo(&chave);
            for pedaco in mensagem.chunks(corte) {
                p.atualizar(pedaco);
            }
            assert_eq!(p.finalizar(), inteiro, "quebrado em {corte} nao bateu");
        }
    }

    #[test]
    fn a_sequencia_nunca_repete_nonce() {
        let s = Sequencia::nova([0xde, 0xad, 0xbe, 0xef]);
        let a = s.nonce(0);
        let b = s.nonce(1);
        assert_ne!(a, b);
        assert_eq!(&a[..4], &[0xde, 0xad, 0xbe, 0xef]);
        // Duas sequencias com prefixos diferentes nao colidem na mesma ordem.
        assert_ne!(Sequencia::nova([1, 0, 0, 0]).nonce(9), s.nonce(9));
    }

    /// A chave sai do PBKDF2 que ja existia -- aqui so se prova que sal
    /// diferente da chave diferente, que e o papel do sal.
    #[test]
    fn sal_diferente_da_chave_diferente() {
        let a = chave_de_senha("segredo", b"sal-a", 1000);
        let b = chave_de_senha("segredo", b"sal-b", 1000);
        assert_ne!(a, b);
        assert_eq!(a, chave_de_senha("segredo", b"sal-a", 1000));
    }
}
