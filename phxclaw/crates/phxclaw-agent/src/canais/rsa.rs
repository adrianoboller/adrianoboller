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
//!
//! **A assinatura** (`ChavePrivada`, desde 09/10/2026) existe para UM uso: o JWT RS256 que a
//! conta de servico do GCP troca por token (`cofres/gcp.rs`). Ali a chave e NOSSA e o expoente
//! e o privado (2048 bits, nao 17), e a reducao bit a bit do verificar custaria ~4.000
//! reducoes de 4.096 passos por assinatura. Por isso a assinatura tem o seu proprio
//! multiplicador -- Montgomery (CIOS) -- e o verificar continua como esta. Onde diverge:
//! - **quadrado e multiplica SEMPRE**, com a escolha por mascara, e a subtracao final do
//!   Montgomery tambem por mascara: o expoente e segredo, e o tempo nao pode depender dos
//!   bits dele;
//! - **sem CRT**: o CRT quadruplica a velocidade e acrescenta p, q, dp, dq e qinv ao codigo
//!   e ao que se guarda em memoria; uma assinatura por hora (o token do GCP vale 1 h) nao
//!   paga isso;
//! - **toda assinatura se confere antes de sair** (`verificar` com o expoente publico): uma
//!   falha de calculo devolvendo assinatura errada vira erro aqui, nao 401 opaco no Google.
//!
//! Conferida contra o vetor da RFC 7515 Apendice A.2 (JWS RS256 com a chave do apendice,
//! assinatura deterministica byte a byte).

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
/// usa `ChavePublica`, que poe a politica antes. Continua `pub` porque os vetores rodam no
/// teste de integracao (`tests/canais.rs`), que so enxerga a API publica -- e por isso o nome
/// carrega o `sem_politica` e a doc o esconde: chamar isto com chave de fora pularia o teto
/// de 4096 bits e o expoente 65537, que sao a defesa contra custo e contra Bleichenbacher.
#[doc(hidden)]
pub fn verificar_pkcs1_sha256_sem_politica(
    n: &[u8],
    e: &[u8],
    msg: &[u8],
    assinatura: &[u8],
) -> bool {
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

// ------------------------------------------------------------------ assinatura

/// Montgomery (CIOS, Koc et al. 1996) sobre o modulo de `Modulo`: `mul(a, b) = a*b*R^-1 mod n`
/// com `R = 2^(64*l)`. So a assinatura usa; o verificar fica na reducao bit a bit.
struct Montgomery {
    n: Grande,
    /// `-n^-1 mod 2^64`.
    n0: u64,
    /// `R^2 mod n`: leva um numero para a forma de Montgomery numa multiplicacao.
    r2: Grande,
}

impl Montgomery {
    fn novo(m: &Modulo) -> Self {
        let l = m.n.len();
        // Newton para o inverso modulo 2^64: cada passo dobra os bits certos (1 -> 64 em 6).
        let mut inv: u64 = 1;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(m.n[0].wrapping_mul(inv)));
        }
        let mut r2 = vec![0u64; 2 * l + 1];
        r2[2 * l] = 1;
        Self {
            n: m.n.clone(),
            n0: inv.wrapping_neg(),
            r2: m.reduzir(&r2),
        }
    }

    /// `a*b*R^-1 mod n`, com `a, b < n` de `l` palavras.
    fn mul(&self, a: &[u64], b: &[u64]) -> Grande {
        let l = self.n.len();
        let mut t = vec![0u64; l + 2];
        for &bi in b.iter().take(l) {
            let mut c: u128 = 0;
            for j in 0..l {
                let s = t[j] as u128 + (a[j] as u128) * (bi as u128) + c;
                t[j] = s as u64;
                c = s >> 64;
            }
            let s = t[l] as u128 + c;
            t[l] = s as u64;
            t[l + 1] = (s >> 64) as u64;
            let m = t[0].wrapping_mul(self.n0);
            let s = t[0] as u128 + (m as u128) * (self.n[0] as u128);
            let mut c = s >> 64;
            for j in 1..l {
                let s = t[j] as u128 + (m as u128) * (self.n[j] as u128) + c;
                t[j - 1] = s as u64;
                c = s >> 64;
            }
            let s = t[l] as u128 + c;
            t[l - 1] = s as u64;
            t[l] = t[l + 1] + (s >> 64) as u64;
            t[l + 1] = 0;
        }
        // t < 2n: subtrai n por mascara (sem desvio pelo valor).
        let mut u = vec![0u64; l];
        let mut emprestimo = 0u64;
        for j in 0..l {
            let (d1, e1) = t[j].overflowing_sub(self.n[j]);
            let (d2, e2) = d1.overflowing_sub(emprestimo);
            u[j] = d2;
            emprestimo = (e1 | e2) as u64;
        }
        let maior = (t[l] | (emprestimo ^ 1)) & 1;
        let mascara = 0u64.wrapping_sub(maior);
        (0..l)
            .map(|j| (u[j] & mascara) | (t[j] & !mascara))
            .collect()
    }

    /// `base^exp mod n`, `base < n`: quadrado e multiplica sempre, escolha por mascara.
    fn potencia(&self, base: &[u64], exp: &[u8]) -> Grande {
        let l = self.n.len();
        let mut um = vec![0u64; l];
        um[0] = 1;
        let x = self.mul(base, &self.r2);
        let mut acc = self.mul(&um, &self.r2);
        for byte in exp {
            for b in (0..8).rev() {
                acc = self.mul(&acc, &acc);
                let t = self.mul(&acc, &x);
                let mascara = 0u64.wrapping_sub(((byte >> b) & 1) as u64);
                for j in 0..l {
                    acc[j] = (t[j] & mascara) | (acc[j] & !mascara);
                }
            }
        }
        self.mul(&acc, &um)
    }
}

/// Chave privada RSA para assinar RS256. Mesma politica da `ChavePublica` (2048 a 4096 bits,
/// `e = 65537`): a chave vem de conta de servico, e qualquer coisa fora disso e arquivo
/// errado, nao chave a acomodar.
pub struct ChavePrivada {
    modulo: Modulo,
    mont: Montgomery,
    /// O expoente privado, big-endian. Zerado no `drop`.
    d: Vec<u8>,
}

impl std::fmt::Debug for ChavePrivada {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChavePrivada({} bits, [REDACTED])", self.modulo.bits())
    }
}

impl Drop for ChavePrivada {
    fn drop(&mut self) {
        for b in self.d.iter_mut() {
            *b = 0;
        }
        std::hint::black_box(&self.d);
    }
}

/// Um TLV DER: (etiqueta, conteudo, resto). So comprimento definido, ate 4 bytes.
fn der(b: &[u8]) -> Result<(u8, &[u8], &[u8]), String> {
    let erro = || "DER truncado ou invalido".to_string();
    let (&tag, resto) = b.split_first().ok_or_else(erro)?;
    let (&c0, resto) = resto.split_first().ok_or_else(erro)?;
    let (len, resto) = if c0 < 0x80 {
        (c0 as usize, resto)
    } else {
        let n = (c0 & 0x7f) as usize;
        if n == 0 || n > 4 || resto.len() < n {
            return Err(erro());
        }
        let len = resto[..n]
            .iter()
            .fold(0usize, |a, x| (a << 8) | *x as usize);
        (len, &resto[n..])
    };
    if resto.len() < len {
        return Err(erro());
    }
    Ok((tag, &resto[..len], &resto[len..]))
}

fn der_esperado(b: &[u8], tag: u8, o_que: &str) -> Result<(Vec<u8>, usize), String> {
    let (t, c, resto) = der(b)?;
    if t != tag {
        return Err(format!("DER: esperava {o_que}"));
    }
    Ok((c.to_vec(), b.len() - resto.len()))
}

/// O OID rsaEncryption (1.2.840.113549.1.1.1), ja codificado.
const OID_RSA: [u8; 9] = [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];

impl ChavePrivada {
    /// `n`, `e` e `d` big-endian (zeros a esquerda tolerados).
    pub fn de_componentes(n: &[u8], e: &[u8], d: &[u8]) -> Result<Self, String> {
        let publica = ChavePublica::nova(n, e)?;
        let d = sem_zeros(d).to_vec();
        if d.is_empty() {
            return Err("expoente privado zero".into());
        }
        let mont = Montgomery::novo(&publica.modulo);
        Ok(Self {
            modulo: publica.modulo,
            mont,
            d,
        })
    }

    /// `RSAPrivateKey` (RFC 8017 Apendice A.1.2): versao, n, e, d, e o resto ignorado.
    fn de_pkcs1(der_: &[u8]) -> Result<Self, String> {
        let (seq, _) = der_esperado(der_, 0x30, "SEQUENCE da RSAPrivateKey")?;
        let mut resto: &[u8] = &seq;
        let mut inteiros = Vec::new();
        for o_que in ["versao", "n", "e", "d"] {
            let (v, usado) = der_esperado(resto, 0x02, o_que)?;
            inteiros.push(v);
            resto = &resto[usado..];
        }
        if sem_zeros(&inteiros[0]).len() > 1 {
            return Err("RSAPrivateKey de versao desconhecida".into());
        }
        Self::de_componentes(&inteiros[1], &inteiros[2], &inteiros[3])
    }

    /// `PrivateKeyInfo` (PKCS#8, RFC 5208): o que a conta de servico do Google traz.
    fn de_pkcs8(der_: &[u8]) -> Result<Self, String> {
        let (seq, _) = der_esperado(der_, 0x30, "SEQUENCE do PrivateKeyInfo")?;
        let (_, usado) = der_esperado(&seq, 0x02, "versao")?;
        let resto = &seq[usado..];
        let (alg, usado) = der_esperado(resto, 0x30, "AlgorithmIdentifier")?;
        let (oid, _) = der_esperado(&alg, 0x06, "OID do algoritmo")?;
        if oid != OID_RSA {
            return Err("a chave nao e RSA (rsaEncryption)".into());
        }
        let (chave, _) = der_esperado(&resto[usado..], 0x04, "OCTET STRING da chave")?;
        Self::de_pkcs1(&chave)
    }

    /// PEM `PRIVATE KEY` (PKCS#8) ou `RSA PRIVATE KEY` (PKCS#1). O erro nunca cita o
    /// conteudo: a chave e segredo.
    pub fn de_pem(pem: &str) -> Result<Self, String> {
        let corpo = |rotulo: &str| -> Option<String> {
            let ini = format!("-----BEGIN {rotulo}-----");
            let fim = format!("-----END {rotulo}-----");
            let a = pem.find(&ini)? + ini.len();
            let b = a + pem[a..].find(&fim)?;
            Some(pem[a..b].chars().filter(|c| !c.is_whitespace()).collect())
        };
        let b64 = |t: String| {
            base64::engine::general_purpose::STANDARD
                .decode(t)
                .map_err(|_| "PEM com base64 invalido".to_string())
        };
        if let Some(t) = corpo("PRIVATE KEY") {
            return Self::de_pkcs8(&b64(t)?);
        }
        if let Some(t) = corpo("RSA PRIVATE KEY") {
            return Self::de_pkcs1(&b64(t)?);
        }
        Err("PEM sem chave privada (PRIVATE KEY ou RSA PRIVATE KEY)".into())
    }

    pub fn bits(&self) -> usize {
        self.modulo.bits()
    }

    /// RSASSA-PKCS1-V1_5-SIGN (RFC 8017 §8.2.1) com SHA-256, conferida antes de sair.
    pub fn assinar(&self, msg: &[u8]) -> Result<Vec<u8>, String> {
        let k = self.modulo.k;
        let em = bloco_esperado(msg, k).ok_or("modulo pequeno demais para SHA-256")?;
        let m = de_bytes(&em, self.modulo.n.len());
        let s = para_bytes(&self.mont.potencia(&m, &self.d), k);
        if !self.modulo.verificar(&EXPOENTE.to_be_bytes()[1..], msg, &s) {
            return Err("a assinatura RSA nao conferiu com a chave publica".into());
        }
        Ok(s)
    }
}
