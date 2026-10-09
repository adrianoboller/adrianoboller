//! Verificacao RSASSA-PKCS1-v1_5 com SHA-256 (RFC 8017 §8.2.2), escrita aqui para conferir
//! o JWT RS256 que o Bot Framework (Teams) e o Google Chat poem no webhook. So VERIFICA:
//! nada aqui assina, e por isso so ha chave publica -- nao e tempo constante, e nao precisa
//! ser (o mesmo argumento do `bip340.rs` para a verificacao).
//!
//! Onde diverge da origem, e por que:
//! - o `bip340.rs` reduz com modulo fixo de 4 palavras; aqui o modulo vem do JWKS de quem
//!   assina, entao o inteiro e um `Vec<u64>` do tamanho da chave (a chave nao e nossa);
//! - a reducao e bit a bit (deslocar e subtrair), e nao Montgomery: com `e = 65537` sao 17
//!   multiplicacoes modulares por mensagem, e o parecer do pesquisador recusou comprar 40x
//!   num custo que fica abaixo do RTT do proprio webhook;
//! - o EMSA-PKCS1-v1_5 nao analisa o ASN.1 da assinatura: MONTA o bloco esperado inteiro
//!   (`00 01 FF..FF 00 || DigestInfo || H`) e compara byte a byte. Analisar e o caminho por
//!   onde entram o «acha o hash no fim» e o BER aceito (as flags `ModifiedPadding`,
//!   `InvalidAsnInPadding` e `BerEncodedPadding` do Wycheproof); montar nao tem por onde.
//!
//! Conferido nos testes contra os vetores oficiais: Wycheproof `rsa_signature_2048_sha256`
//! (259 casos) e NIST CAVP SigVer15 SHA-256 (54 casos, modulos de 1024, 2048 e 3072).

use base64::Engine;
use sha2::{Digest, Sha256};

/// O prefixo DER do DigestInfo de SHA-256 (RFC 8017 §9.2, nota 1).
const DIGEST_INFO_SHA256: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

/// O tamanho de chave que o JWKS pode trazer. Abaixo de 2048 nao e chave que alguem ainda
/// emita para assinar token; acima de 4096 cada verificacao fica cara o bastante para virar
/// porta de negacao de servico (o modulo e de quem publica o JWKS, nao nosso).
pub const BITS_MIN: usize = 2048;
pub const BITS_MAX: usize = 4096;
/// O unico expoente aceito do JWKS: os dois servicos publicam `AQAB` hoje (medido pelo
/// pesquisador: 225 chaves do Bot Framework e 4 do Google Chat), e expoente pequeno
/// (`e = 3`) e o que torna a falsificacao de Bleichenbacher praticavel quando alguem erra o
/// padding.
pub const EXPOENTE: u32 = 65_537;

/// Inteiro sem sinal, palavras de 64 bits, a MENOS significativa primeiro.
type Grande = Vec<u64>;

/// Bytes big-endian -> `limbs` palavras (o tamanho cabe: quem chama ja conferiu).
fn de_bytes(b: &[u8], limbs: usize) -> Grande {
    let mut r = vec![0u64; limbs];
    for (i, byte) in b.iter().rev().enumerate() {
        r[i / 8] |= (*byte as u64) << (8 * (i % 8));
    }
    r
}

/// I2OSP: o inteiro em exatamente `k` bytes big-endian (o valor e menor que o modulo, que
/// tem `k` bytes, entao sempre cabe).
fn para_bytes(a: &[u64], k: usize) -> Vec<u8> {
    (0..k)
        .rev()
        .map(|i| {
            let limb = a.get(i / 8).copied().unwrap_or(0);
            (limb >> (8 * (i % 8))) as u8
        })
        .collect()
}

fn maior_ou_igual(a: &[u64], b: &[u64]) -> bool {
    let n = a.len().max(b.len());
    for i in (0..n).rev() {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    true
}

/// `a -= b`, com `a >= b` garantido por quem chama.
fn subtrair(a: &mut [u64], b: &[u64]) {
    let mut emprestimo = 0u64;
    for (i, x) in a.iter_mut().enumerate() {
        let y = b.get(i).copied().unwrap_or(0);
        let (d1, e1) = x.overflowing_sub(y);
        let (d2, e2) = d1.overflowing_sub(emprestimo);
        *x = d2;
        emprestimo = (e1 | e2) as u64;
    }
}

/// Produto escolar: `a.len() + b.len()` palavras.
fn multiplicar(a: &[u64], b: &[u64]) -> Grande {
    let mut r = vec![0u64; a.len() + b.len()];
    for (i, x) in a.iter().enumerate() {
        let mut vai = 0u128;
        for (j, y) in b.iter().enumerate() {
            let t = (*x as u128) * (*y as u128) + r[i + j] as u128 + vai;
            r[i + j] = t as u64;
            vai = t >> 64;
        }
        r[i + b.len()] = vai as u64;
    }
    r
}

/// O modulo `n` (impar, sem zeros a esquerda) e o tamanho dele em bytes.
struct Modulo {
    n: Grande,
    k: usize,
}

impl Modulo {
    /// `None` para modulo zero ou par: nao e modulo RSA, e a reducao abaixo nao precisa
    /// tratar o que nunca deveria chegar nela.
    fn novo(n: &[u8]) -> Option<Self> {
        let n = sem_zeros(n);
        if n.is_empty() || n[n.len() - 1] & 1 == 0 {
            return None;
        }
        Some(Self {
            n: de_bytes(n, n.len().div_ceil(8)),
            k: n.len(),
        })
    }

    fn bits(&self) -> usize {
        let topo = self.n[self.n.len() - 1];
        (self.n.len() - 1) * 64 + (64 - topo.leading_zeros() as usize)
    }

    /// `x mod n` por deslocar e subtrair, um bit de cada vez do mais alto: o resto fica
    /// sempre abaixo de `n`, entao uma palavra a mais basta para o deslocamento.
    fn reduzir(&self, x: &[u64]) -> Grande {
        let l = self.n.len();
        let mut r = vec![0u64; l + 1];
        for i in (0..x.len() * 64).rev() {
            let bit = (x[i / 64] >> (i % 64)) & 1;
            let mut vai = bit;
            for p in r.iter_mut() {
                let novo = (*p << 1) | vai;
                vai = *p >> 63;
                *p = novo;
            }
            if maior_ou_igual(&r, &self.n) {
                subtrair(&mut r, &self.n);
            }
        }
        r.truncate(l);
        r
    }

    fn mul_mod(&self, a: &[u64], b: &[u64]) -> Grande {
        self.reduzir(&multiplicar(a, b))
    }

    /// `s^e mod n`, quadrado e multiplica do bit mais alto de `e` para baixo.
    fn potencia(&self, s: &[u64], e: &[u8]) -> Grande {
        let mut r: Option<Grande> = None;
        for byte in e {
            for b in (0..8).rev() {
                if let Some(x) = r.take() {
                    r = Some(self.mul_mod(&x, &x));
                }
                if (byte >> b) & 1 == 1 {
                    r = Some(match r.take() {
                        Some(x) => self.mul_mod(&x, s),
                        None => s.to_vec(),
                    });
                }
            }
        }
        // e = 0 da 1; nao chega aqui pela politica, mas o primitivo responde certo.
        r.unwrap_or_else(|| {
            let mut um = vec![0u64; self.n.len()];
            um[0] = 1;
            self.reduzir(&um)
        })
    }
}

fn sem_zeros(b: &[u8]) -> &[u8] {
    let i = b.iter().position(|x| *x != 0).unwrap_or(b.len());
    &b[i..]
}

/// EMSA-PKCS1-v1_5-ENCODE (RFC 8017 §9.2) para SHA-256: `None` se `k` nao comporta o
/// bloco (k < tLen + 11).
fn bloco_esperado(msg: &[u8], k: usize) -> Option<Vec<u8>> {
    let t_len = DIGEST_INFO_SHA256.len() + 32;
    if k < t_len + 11 {
        return None;
    }
    let mut em = Vec::with_capacity(k);
    em.extend_from_slice(&[0x00, 0x01]);
    em.resize(k - t_len - 1, 0xff);
    em.push(0x00);
    em.extend_from_slice(&DIGEST_INFO_SHA256);
    em.extend_from_slice(&Sha256::digest(msg));
    Some(em)
}

impl Modulo {
    /// RSASSA-PKCS1-V1_5-VERIFY (RFC 8017 §8.2.2), passo a passo.
    fn verificar(&self, e: &[u8], msg: &[u8], assinatura: &[u8]) -> bool {
        // 1. A assinatura tem exatamente k bytes -- nem um zero a mais, nem a menos.
        if assinatura.len() != self.k {
            return false;
        }
        // 2. RSAVP1: s < n, m = s^e mod n, EM = I2OSP(m, k).
        let s = de_bytes(assinatura, self.n.len());
        if maior_ou_igual(&s, &self.n) {
            return false;
        }
        let em = para_bytes(&self.potencia(&s, e), self.k);
        // 3 e 4. O bloco esperado inteiro, comparado inteiro.
        match bloco_esperado(msg, self.k) {
            Some(esperado) => super::cripto::iguais(&em, &esperado),
            None => false,
        }
    }
}

/// O primitivo SEM politica de chave (qualquer tamanho, qualquer expoente), para os vetores
/// oficiais, que trazem 1024 bits e expoentes que o JWKS nunca traria. Quem confere token
/// usa `ChavePublica`, que poe a politica antes.
pub fn verificar_pkcs1_sha256(n: &[u8], e: &[u8], msg: &[u8], assinatura: &[u8]) -> bool {
    Modulo::novo(n).is_some_and(|m| m.verificar(e, msg, assinatura))
}

/// Chave publica RSA que passou pela politica: 2048 a 4096 bits, `e = 65537`.
pub struct ChavePublica {
    modulo: Modulo,
}

impl std::fmt::Debug for ChavePublica {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChavePublica({} bits)", self.bits())
    }
}

impl ChavePublica {
    /// `n` e `e` como inteiros big-endian sem sinal (zeros a esquerda sao tolerados: ha
    /// biblioteca que os poe, e a RFC 7518 §6.3.1.1 registra isso).
    pub fn nova(n: &[u8], e: &[u8]) -> Result<Self, String> {
        let modulo = Modulo::novo(n).ok_or("modulo RSA zero ou par")?;
        let bits = modulo.bits();
        if !(BITS_MIN..=BITS_MAX).contains(&bits) {
            return Err(format!(
                "modulo de {bits} bits fora de {BITS_MIN}..={BITS_MAX}"
            ));
        }
        if sem_zeros(e) != &EXPOENTE.to_be_bytes()[1..] {
            return Err(format!("expoente diferente de {EXPOENTE}"));
        }
        Ok(Self { modulo })
    }

    /// Da forma JWK (RFC 7518 §6.3.1): `n` e `e` em base64url sem preenchimento.
    pub fn de_jwk(n: &str, e: &str) -> Result<Self, String> {
        let d = |campo: &str, v: &str| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(v)
                .map_err(|x| format!("JWK: {campo} nao e base64url: {x}"))
        };
        Self::nova(&d("n", n)?, &d("e", e)?)
    }

    pub fn bits(&self) -> usize {
        self.modulo.bits()
    }

    pub fn verificar(&self, msg: &[u8], assinatura: &[u8]) -> bool {
        self.modulo
            .verificar(&EXPOENTE.to_be_bytes()[1..], msg, assinatura)
    }
}
