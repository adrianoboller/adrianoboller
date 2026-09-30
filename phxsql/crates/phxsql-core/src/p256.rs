//! A curva NIST P-256 (secp256r1): ECDSA com SHA-256 e ECDH, sem dependencia
//! externa.
//!
//! # Para que serve aqui
//!
//! TLS 1.3 escrito nesta casa (decisao do dono, 30/09/2026). O navegador so
//! aceita certificado assinado por RSA ou ECDSA -- Ed25519 em certificado nenhum
//! dos grandes aceita --, e a RFC 8446 §9.1 poe `ecdsa_secp256r1_sha256` e o
//! grupo `secp256r1` entre os obrigatorios. Esta e a peca que faltava: o resto
//! da criptografia do TLS (X25519, ChaCha20-Poly1305, HKDF, SHA-256) ja estava
//! escrito e conferido.
//!
//! # De onde veio, e como se sabe que esta certo
//!
//! A curva e a do FIPS 186-4 §D.1.2.3. A aritmetica e a de Montgomery (CIOS),
//! a mesma rotina para o campo `p` e para a ordem `n`. A soma de pontos e a
//! **completa** de Renes, Costello e Batina (2016, Algoritmo 4, a = -3): ela nao
//! tem caso especial para ponto no infinito nem para P = Q, e e isso que deixa a
//! multiplicacao escalar ser uma escada de Montgomery sem desvio que dependa do
//! segredo. O `k` da assinatura e o deterministico da RFC 6979 §3.2, e os
//! testes conferem chave publica, `k`, `r` e `s` contra o Apendice A.2.5 dela --
//! copiado do texto oficial, nao de memoria --, e a interoperabilidade contra o
//! OpenSSL (`p256_conversa_com_o_openssl`, quando o binario existe).
//!
//! # O que ela promete, e o que nao
//!
//! * A multiplicacao pelo escalar SECRETO (chave privada, `k`) e uma escada com
//!   troca condicional por mascara: a sequencia de operacoes nao depende dos
//!   bits. A multiplicacao de Montgomery tambem nao desvia. Isso NAO e uma
//!   auditoria de tempo constante -- o compilador pode reintroduzir desvio, e
//!   ninguem mediu a cache --, e esta escrito para ninguem ler mais do que isto.
//! * A verificacao lida so com dado publico e nao se preocupa com tempo.

use crate::hash::{hmac_sha256, sha256};

type Limbs = [u64; 4];

/// Um corpo de inteiros modulo `m` em forma de Montgomery (R = 2^256).
struct Corpo {
    m: Limbs,
    /// `-m^-1 mod 2^64`.
    m0: u64,
    /// `R^2 mod m`, para entrar na forma de Montgomery.
    r2: Limbs,
}

const fn de_be(b: &[u8; 32]) -> Limbs {
    let mut l = [0u64; 4];
    let mut i = 0;
    while i < 4 {
        let mut v = 0u64;
        let mut j = 0;
        while j < 8 {
            v = (v << 8) | b[(3 - i) * 8 + j] as u64;
            j += 1;
        }
        l[i] = v;
        i += 1;
    }
    l
}

fn para_be(l: &Limbs) -> [u8; 32] {
    let mut b = [0u8; 32];
    for i in 0..4 {
        b[(3 - i) * 8..(4 - i) * 8].copy_from_slice(&l[i].to_be_bytes());
    }
    b
}

const fn hex32(s: &str) -> [u8; 32] {
    let s = s.as_bytes();
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        let alto = digito(s[2 * i]);
        let baixo = digito(s[2 * i + 1]);
        out[i] = (alto << 4) | baixo;
        i += 1;
    }
    out
}

const fn digito(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("digito hexadecimal invalido numa constante"),
    }
}

/// `a + b`, com o vai-um.
#[inline]
fn somar(a: &Limbs, b: &Limbs) -> (Limbs, u64) {
    let mut r = [0u64; 4];
    let mut c = 0u64;
    for i in 0..4 {
        let s = a[i] as u128 + b[i] as u128 + c as u128;
        r[i] = s as u64;
        c = (s >> 64) as u64;
    }
    (r, c)
}

/// `a - b`, com o empresta-um (1 quando `a < b`).
#[inline]
fn subtrair(a: &Limbs, b: &Limbs) -> (Limbs, u64) {
    let mut r = [0u64; 4];
    let mut emprestimo = 0u64;
    for i in 0..4 {
        let (d1, e1) = a[i].overflowing_sub(b[i]);
        let (d2, e2) = d1.overflowing_sub(emprestimo);
        r[i] = d2;
        emprestimo = (e1 | e2) as u64;
    }
    (r, emprestimo)
}

/// `mascara ? a : b`, sem desvio. `mascara` e 0 ou `u64::MAX`.
#[inline]
fn escolher(mascara: u64, a: &Limbs, b: &Limbs) -> Limbs {
    let mut r = [0u64; 4];
    for i in 0..4 {
        r[i] = (a[i] & mascara) | (b[i] & !mascara);
    }
    r
}

impl Corpo {
    fn novo(m: Limbs) -> Corpo {
        // Newton para o inverso modulo 2^64: cada passo dobra os bits certos.
        let mut inv = 1u64;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(m[0].wrapping_mul(inv)));
        }
        let mut c = Corpo {
            m,
            m0: inv.wrapping_neg(),
            r2: [0; 4],
        };
        // R^2 mod m: comeca em 1 e dobra 512 vezes -- conta de uma vez so, na
        // construcao, e sem constante digitada para errar.
        let mut x: Limbs = [1, 0, 0, 0];
        for _ in 0..512 {
            x = c.somar(&x, &x);
        }
        c.r2 = x;
        c
    }

    fn somar(&self, a: &Limbs, b: &Limbs) -> Limbs {
        let (s, c) = somar(a, b);
        let (t, e) = subtrair(&s, &self.m);
        // Usa `t` quando a soma estourou 2^256 ou quando `s >= m`.
        let usar_t = (c | (e ^ 1)).wrapping_neg();
        escolher(usar_t, &t, &s)
    }

    fn subtrair(&self, a: &Limbs, b: &Limbs) -> Limbs {
        let (d, e) = subtrair(a, b);
        let (t, _) = somar(&d, &self.m);
        escolher(e.wrapping_neg(), &t, &d)
    }

    /// Multiplicacao de Montgomery: `a * b * R^-1 mod m` (CIOS).
    fn mul(&self, a: &Limbs, b: &Limbs) -> Limbs {
        let m = &self.m;
        let mut t = [0u64; 6];
        for &bi in b {
            let mut c = 0u128;
            for j in 0..4 {
                let v = t[j] as u128 + a[j] as u128 * bi as u128 + c;
                t[j] = v as u64;
                c = v >> 64;
            }
            let v = t[4] as u128 + c;
            t[4] = v as u64;
            t[5] = (v >> 64) as u64;

            let q = t[0].wrapping_mul(self.m0);
            let v = t[0] as u128 + q as u128 * m[0] as u128;
            let mut c = v >> 64;
            for j in 1..4 {
                let v = t[j] as u128 + q as u128 * m[j] as u128 + c;
                t[j - 1] = v as u64;
                c = v >> 64;
            }
            let v = t[4] as u128 + c;
            t[3] = v as u64;
            t[4] = t[5] + (v >> 64) as u64;
        }
        let r = [t[0], t[1], t[2], t[3]];
        let (s, e) = subtrair(&r, m);
        // `r` pode passar de `m` uma vez; o bit de cima (t[4]) conta tambem.
        let usar_s = (t[4] | (e ^ 1)).wrapping_neg();
        escolher(usar_s, &s, &r)
    }

    fn entrar(&self, a: &Limbs) -> Limbs {
        self.mul(a, &self.r2)
    }

    fn sair(&self, a: &Limbs) -> Limbs {
        self.mul(a, &[1, 0, 0, 0])
    }

    /// Reduz um inteiro de 256 bits: aqui `m > 2^255`, entao uma subtracao
    /// basta.
    fn reduzir(&self, a: &Limbs) -> Limbs {
        let (s, e) = subtrair(a, &self.m);
        escolher(e.wrapping_neg(), a, &s)
    }

    /// `a^e` em forma de Montgomery. O expoente e PUBLICO (m - 2), entao a
    /// sequencia de operacoes nao depende de segredo nenhum.
    fn potencia(&self, a: &Limbs, e: &Limbs) -> Limbs {
        let mut r = self.entrar(&[1, 0, 0, 0]);
        for i in (0..256).rev() {
            r = self.mul(&r, &r);
            if (e[i / 64] >> (i % 64)) & 1 == 1 {
                r = self.mul(&r, a);
            }
        }
        r
    }

    /// Inverso por Fermat: `a^(m-2)`.
    fn inverso(&self, a: &Limbs) -> Limbs {
        let (e, _) = subtrair(&self.m, &[2, 0, 0, 0]);
        self.potencia(a, &e)
    }

    fn eh_zero(a: &Limbs) -> bool {
        (a[0] | a[1] | a[2] | a[3]) == 0
    }
}

const P: [u8; 32] = hex32("FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF");
const N: [u8; 32] = hex32("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551");
const B: [u8; 32] = hex32("5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B");
const GX: [u8; 32] = hex32("6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296");
const GY: [u8; 32] = hex32("4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5");

/// Os dois corpos e as constantes da curva ja em forma de Montgomery.
struct Curva {
    fp: Corpo,
    fnn: Corpo,
    b: Limbs,
    g: Ponto,
}

fn curva() -> &'static Curva {
    static CURVA: std::sync::OnceLock<Curva> = std::sync::OnceLock::new();
    CURVA.get_or_init(|| {
        let fp = Corpo::novo(de_be(&P));
        let fnn = Corpo::novo(de_be(&N));
        let b = fp.entrar(&de_be(&B));
        let g = Ponto {
            x: fp.entrar(&de_be(&GX)),
            y: fp.entrar(&de_be(&GY)),
            z: fp.entrar(&[1, 0, 0, 0]),
        };
        Curva { fp, fnn, b, g }
    })
}

/// Ponto em coordenadas projetivas homogeneas (x = X/Z, y = Y/Z), com os tres
/// no corpo `p` em forma de Montgomery. O infinito e (0 : 1 : 0).
#[derive(Clone, Copy)]
struct Ponto {
    x: Limbs,
    y: Limbs,
    z: Limbs,
}

impl Ponto {
    fn infinito(c: &Curva) -> Ponto {
        Ponto {
            x: [0; 4],
            y: c.fp.entrar(&[1, 0, 0, 0]),
            z: [0; 4],
        }
    }

    /// Soma COMPLETA para a = -3 (Renes-Costello-Batina 2016, Algoritmo 4).
    /// Vale para P = Q e para o infinito, sem desvio -- a escada depende disso.
    fn somar(&self, q: &Ponto, c: &Curva) -> Ponto {
        let f = &c.fp;
        let (x1, y1, z1) = (&self.x, &self.y, &self.z);
        let (x2, y2, z2) = (&q.x, &q.y, &q.z);
        let mut t0 = f.mul(x1, x2);
        let mut t1 = f.mul(y1, y2);
        let mut t2 = f.mul(z1, z2);
        let mut t3 = f.somar(x1, y1);
        let mut t4 = f.somar(x2, y2);
        t3 = f.mul(&t3, &t4);
        t4 = f.somar(&t0, &t1);
        t3 = f.subtrair(&t3, &t4);
        t4 = f.somar(y1, z1);
        let mut x3 = f.somar(y2, z2);
        t4 = f.mul(&t4, &x3);
        x3 = f.somar(&t1, &t2);
        t4 = f.subtrair(&t4, &x3);
        x3 = f.somar(x1, z1);
        let mut y3 = f.somar(x2, z2);
        x3 = f.mul(&x3, &y3);
        y3 = f.somar(&t0, &t2);
        y3 = f.subtrair(&x3, &y3);
        let mut z3 = f.mul(&c.b, &t2);
        x3 = f.subtrair(&y3, &z3);
        z3 = f.somar(&x3, &x3);
        x3 = f.somar(&x3, &z3);
        z3 = f.subtrair(&t1, &x3);
        x3 = f.somar(&t1, &x3);
        y3 = f.mul(&c.b, &y3);
        t1 = f.somar(&t2, &t2);
        t2 = f.somar(&t1, &t2);
        y3 = f.subtrair(&y3, &t2);
        y3 = f.subtrair(&y3, &t0);
        t1 = f.somar(&y3, &y3);
        y3 = f.somar(&t1, &y3);
        t1 = f.somar(&t0, &t0);
        t0 = f.somar(&t1, &t0);
        t0 = f.subtrair(&t0, &t2);
        t1 = f.mul(&t4, &y3);
        t2 = f.mul(&t0, &y3);
        y3 = f.mul(&x3, &z3);
        y3 = f.somar(&y3, &t2);
        x3 = f.mul(&t3, &x3);
        x3 = f.subtrair(&x3, &t1);
        z3 = f.mul(&t4, &z3);
        t1 = f.mul(&t3, &t0);
        z3 = f.somar(&z3, &t1);
        Ponto {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    /// `k * P` pela escada de Montgomery: 256 passos, uma soma e uma dobra
    /// cada, com a troca por mascara. Nenhum desvio depende de `k`.
    fn vezes(&self, k: &Limbs, c: &Curva) -> Ponto {
        let mut r0 = Ponto::infinito(c);
        let mut r1 = *self;
        for i in (0..256).rev() {
            let bit = (k[i / 64] >> (i % 64)) & 1;
            trocar(bit, &mut r0, &mut r1);
            r1 = r0.somar(&r1, c);
            r0 = r0.somar(&r0, c);
            trocar(bit, &mut r0, &mut r1);
        }
        r0
    }

    /// `(x, y)` fora da forma de Montgomery, ou `None` no infinito.
    fn afim(&self, c: &Curva) -> Option<(Limbs, Limbs)> {
        if Corpo::eh_zero(&self.z) {
            return None;
        }
        let zi = c.fp.inverso(&self.z);
        Some((
            c.fp.sair(&c.fp.mul(&self.x, &zi)),
            c.fp.sair(&c.fp.mul(&self.y, &zi)),
        ))
    }

    /// O ponto `(x, y)` afim, conferido na curva: `y^2 = x^3 - 3x + b`.
    fn de_afim(x: &Limbs, y: &Limbs, c: &Curva) -> Option<Ponto> {
        let f = &c.fp;
        let (_, ex) = subtrair(x, &f.m);
        let (_, ey) = subtrair(y, &f.m);
        if ex == 0 || ey == 0 {
            return None; // coordenada >= p
        }
        let xm = f.entrar(x);
        let ym = f.entrar(y);
        let y2 = f.mul(&ym, &ym);
        let x3 = f.mul(&f.mul(&xm, &xm), &xm);
        let tres_x = f.somar(&f.somar(&xm, &xm), &xm);
        let direita = f.somar(&f.subtrair(&x3, &tres_x), &c.b);
        if f.sair(&y2) != f.sair(&direita) {
            return None;
        }
        Some(Ponto {
            x: xm,
            y: ym,
            z: f.entrar(&[1, 0, 0, 0]),
        })
    }
}

fn trocar(bit: u64, a: &mut Ponto, b: &mut Ponto) {
    let m = bit.wrapping_neg();
    for (u, v) in [
        (&mut a.x, &mut b.x),
        (&mut a.y, &mut b.y),
        (&mut a.z, &mut b.z),
    ] {
        for i in 0..4 {
            let t = (u[i] ^ v[i]) & m;
            u[i] ^= t;
            v[i] ^= t;
        }
    }
}

/// Chave privada valida: `1 <= d < n`.
fn escalar_valido(d: &Limbs) -> bool {
    let (_, e) = subtrair(d, &de_be(&N));
    e == 1 && !Corpo::eh_zero(d)
}

/// A chave publica de `d`, no formato nao comprimido da SEC 1 §2.3.3:
/// `04 || X || Y`, 65 bytes. `None` se `d` nao esta em `[1, n-1]`.
pub fn chave_publica(privada: &[u8; 32]) -> Option<[u8; 65]> {
    let c = curva();
    let d = de_be(privada);
    if !escalar_valido(&d) {
        return None;
    }
    let (x, y) = c.g.vezes(&d, c).afim(c)?;
    let mut out = [0u8; 65];
    out[0] = 4;
    out[1..33].copy_from_slice(&para_be(&x));
    out[33..].copy_from_slice(&para_be(&y));
    Some(out)
}

/// Uma chave privada nova, de `bytes_aleatorios`, sorteada ate cair em
/// `[1, n-1]` -- a rejeicao e a forma sem vies (FIPS 186-4 §B.4.2).
pub fn gerar_privada() -> [u8; 32] {
    loop {
        let b = crate::senha::bytes_aleatorios(32);
        let mut d = [0u8; 32];
        d.copy_from_slice(&b);
        if escalar_valido(&de_be(&d)) {
            return d;
        }
    }
}

fn ponto_publico(publica: &[u8]) -> Option<Ponto> {
    if publica.len() != 65 || publica[0] != 4 {
        return None;
    }
    let mut x = [0u8; 32];
    let mut y = [0u8; 32];
    x.copy_from_slice(&publica[1..33]);
    y.copy_from_slice(&publica[33..]);
    Ponto::de_afim(&de_be(&x), &de_be(&y), curva())
}

/// ECDH: a coordenada `x` de `d * Q` (RFC 8446 §7.4.2 usa so ela). `None` se
/// `Q` nao esta na curva, se `d` e invalido, ou se o resultado e o infinito.
pub fn ecdh(privada: &[u8; 32], publica_do_outro: &[u8]) -> Option<[u8; 32]> {
    let c = curva();
    let d = de_be(privada);
    if !escalar_valido(&d) {
        return None;
    }
    let q = ponto_publico(publica_do_outro)?;
    let (x, _) = q.vezes(&d, c).afim(c)?;
    Some(para_be(&x))
}

/// O `k` deterministico da RFC 6979 §3.2, com HMAC-SHA-256 e `qlen = 256`.
fn k_deterministico(d: &[u8; 32], h: &[u8; 32]) -> impl Iterator<Item = Limbs> {
    let fnn = &curva().fnn;
    // bits2octets(h1) = int2octets(bits2int(h1) mod q).
    let h_mod = para_be(&fnn.reduzir(&de_be(h)));
    let mut v = [1u8; 32];
    let mut k = [0u8; 32];
    let passo = |sep: u8, com_dado: bool, k: &mut [u8; 32], v: &mut [u8; 32]| {
        let mut m = Vec::with_capacity(97);
        m.extend_from_slice(v);
        m.push(sep);
        if com_dado {
            m.extend_from_slice(d);
            m.extend_from_slice(&h_mod);
        }
        *k = hmac_sha256(k, &m);
        *v = hmac_sha256(k, v);
    };
    passo(0, true, &mut k, &mut v);
    passo(1, true, &mut k, &mut v);
    std::iter::from_fn(move || loop {
        v = hmac_sha256(&k, &v);
        let cand = de_be(&v);
        // Prepara o proximo antes de devolver: quem pede outro candidato
        // (r ou s deu zero) recebe o da §3.2 passo h.3.
        let aceito = escalar_valido(&cand);
        let mut m = v.to_vec();
        m.push(0);
        let proximo_k = hmac_sha256(&k, &m);
        if aceito {
            let devolver = cand;
            k = proximo_k;
            v = hmac_sha256(&k, &v);
            return Some(devolver);
        }
        k = proximo_k;
        v = hmac_sha256(&k, &v);
    })
}

/// Assinatura ECDSA de um RESUMO SHA-256 ja calculado: `(r, s)`, 32 bytes cada.
/// `None` so para chave privada fora de `[1, n-1]`.
pub fn assinar_resumo(privada: &[u8; 32], resumo: &[u8; 32]) -> Option<([u8; 32], [u8; 32])> {
    let c = curva();
    let fnn = &c.fnn;
    let d = de_be(privada);
    if !escalar_valido(&d) {
        return None;
    }
    let e = fnn.entrar(&fnn.reduzir(&de_be(resumo)));
    let dm = fnn.entrar(&d);
    for k in k_deterministico(privada, resumo) {
        let Some((x, _)) = c.g.vezes(&k, c).afim(c) else {
            continue;
        };
        let r = fnn.reduzir(&x);
        if Corpo::eh_zero(&r) {
            continue;
        }
        let km = fnn.entrar(&k);
        let ki = fnn.inverso(&km);
        let rm = fnn.entrar(&r);
        let s = fnn.sair(&fnn.mul(&ki, &fnn.somar(&e, &fnn.mul(&rm, &dm))));
        if Corpo::eh_zero(&s) {
            continue;
        }
        return Some((para_be(&r), para_be(&s)));
    }
    None
}

/// Assina `mensagem` com ECDSA-P256-SHA256.
pub fn assinar(privada: &[u8; 32], mensagem: &[u8]) -> Option<([u8; 32], [u8; 32])> {
    assinar_resumo(privada, &sha256(mensagem))
}

/// Confere uma assinatura `(r, s)` sobre o RESUMO SHA-256, contra a chave
/// publica `04 || X || Y`.
pub fn verificar_resumo(publica: &[u8], resumo: &[u8; 32], r: &[u8; 32], s: &[u8; 32]) -> bool {
    let c = curva();
    let fnn = &c.fnn;
    let Some(q) = ponto_publico(publica) else {
        return false;
    };
    let (r, s) = (de_be(r), de_be(s));
    if !escalar_valido(&r) || !escalar_valido(&s) {
        return false;
    }
    let e = fnn.entrar(&fnn.reduzir(&de_be(resumo)));
    let w = fnn.inverso(&fnn.entrar(&s));
    let u1 = fnn.sair(&fnn.mul(&e, &w));
    let u2 = fnn.sair(&fnn.mul(&fnn.entrar(&r), &w));
    let soma = c.g.vezes(&u1, c).somar(&q.vezes(&u2, c), c);
    match soma.afim(c) {
        Some((x, _)) => fnn.reduzir(&x) == r,
        None => false,
    }
}

/// Confere uma assinatura ECDSA-P256-SHA256 sobre `mensagem`.
pub fn verificar(publica: &[u8], mensagem: &[u8], r: &[u8; 32], s: &[u8; 32]) -> bool {
    verificar_resumo(publica, &sha256(mensagem), r, s)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn h(s: &str) -> [u8; 32] {
        hex32(s)
    }

    /// RFC 6979 Apendice A.2.5 -- a chave do vetor.
    const X: &str = "C9AFA9D845BA75166B5C215767B1D6934E50C3DB36E89B127B8A622B120F6721";

    #[test]
    fn chave_publica_do_vetor_da_rfc_6979() {
        let q = chave_publica(&h(X)).unwrap();
        assert_eq!(
            q[1..33],
            h("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6")
        );
        assert_eq!(
            q[33..],
            h("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299")
        );
    }

    #[test]
    fn k_deterministico_do_vetor_da_rfc_6979() {
        let k = k_deterministico(&h(X), &sha256(b"sample")).next().unwrap();
        assert_eq!(
            para_be(&k),
            h("A6E3C57DD01ABE90086538398355DD4C3B17AA873382B0F24D6129493D8AAD60")
        );
        let k = k_deterministico(&h(X), &sha256(b"test")).next().unwrap();
        assert_eq!(
            para_be(&k),
            h("D16B6AE827F17175E040871A1C7EC3500192C4C92677336EC2537ACAEE0008E0")
        );
    }

    #[test]
    fn assinatura_do_vetor_da_rfc_6979() {
        let (r, s) = assinar(&h(X), b"sample").unwrap();
        assert_eq!(
            r,
            h("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716")
        );
        assert_eq!(
            s,
            h("F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8")
        );
        let (r, s) = assinar(&h(X), b"test").unwrap();
        assert_eq!(
            r,
            h("F1ABB023518351CD71D881567B1EA663ED3EFCF6C5132B354F28D3B0B7D38367")
        );
        assert_eq!(
            s,
            h("019F4113742A2B14BD25926B49C649155F267E60D3814B4C0CC84250E46F0083")
        );
    }

    #[test]
    fn verifica_a_propria_e_recusa_a_adulterada() {
        let d = h(X);
        let q = chave_publica(&d).unwrap();
        let (r, mut s) = assinar(&d, b"sample").unwrap();
        assert!(verificar(&q, b"sample", &r, &s));
        assert!(!verificar(&q, b"sampla", &r, &s), "mensagem trocada passou");
        s[31] ^= 1;
        assert!(!verificar(&q, b"sample", &r, &s), "s adulterado passou");
        let mut fora = q;
        fora[64] ^= 1;
        assert!(
            !verificar(&fora, b"sample", &r, &s),
            "ponto fora da curva passou"
        );
    }

    #[test]
    fn ecdh_concorda_dos_dois_lados() {
        let a = gerar_privada();
        let b = gerar_privada();
        let (qa, qb) = (chave_publica(&a).unwrap(), chave_publica(&b).unwrap());
        assert_eq!(ecdh(&a, &qb).unwrap(), ecdh(&b, &qa).unwrap());
    }

    /// O ECDH com ponto FORA da curva e o ataque de curva invalida: o
    /// multiplicador opera noutra curva, de ordem pequena, e cada resposta
    /// entrega pedacos da chave privada. A recusa tem de ser do ECDH, e nao
    /// da assinatura -- adulterar o ponto faz a assinatura falhar de qualquer
    /// jeito, e um teste por ali passaria com a conferencia arrancada (medido).
    #[test]
    fn ecdh_recusa_ponto_fora_da_curva() {
        let q = chave_publica(&h(X)).unwrap();
        let mut fora = q;
        fora[64] ^= 1;
        assert!(
            ecdh(&gerar_privada(), &fora).is_none(),
            "ECDH aceitou ponto fora da curva"
        );
        let mut comprimido = q;
        comprimido[0] = 2;
        assert!(ecdh(&gerar_privada(), &comprimido).is_none());
        assert!(ecdh(&gerar_privada(), &q).is_some());
    }

    #[test]
    fn escalar_fora_da_faixa_recusa() {
        assert!(chave_publica(&[0u8; 32]).is_none());
        assert!(chave_publica(&N).is_none());
    }

    /// Interoperabilidade: o OpenSSL verifica o que esta casa assina, e esta
    /// casa verifica o que o OpenSSL assina. Sem o binario, o teste diz que
    /// nao rodou em vez de passar calado.
    #[test]
    fn p256_conversa_com_o_openssl() {
        use std::process::Command;
        if Command::new("openssl").arg("version").output().is_err() {
            eprintln!("p256_conversa_com_o_openssl: sem openssl nesta maquina, NAO conferido");
            return;
        }
        let guarda = DirDoTeste::novo();
        let dir = guarda.0.clone();
        let d = gerar_privada();
        let q = chave_publica(&d).unwrap();
        // A chave publica em SubjectPublicKeyInfo DER, para o `pkeyutl`.
        let mut spki = hex_para_vec("3059301306072a8648ce3d020106082a8648ce3d030107034200");
        spki.extend_from_slice(&q);
        std::fs::write(dir.join("pub.der"), &spki).unwrap();
        let msg = b"mensagem de interoperabilidade";
        std::fs::write(dir.join("msg"), msg).unwrap();
        let (r, s) = assinar(&d, msg).unwrap();
        std::fs::write(dir.join("sig"), der_da_assinatura(&r, &s)).unwrap();
        let st = Command::new("openssl")
            .args(["dgst", "-sha256", "-verify"])
            .arg(dir.join("pub.der"))
            .args(["-keyform", "DER", "-signature"])
            .arg(dir.join("sig"))
            .arg(dir.join("msg"))
            .output()
            .unwrap();
        assert!(
            st.status.success(),
            "o OpenSSL recusou a assinatura desta casa: {}",
            String::from_utf8_lossy(&st.stdout)
        );

        // O outro sentido: chave e assinatura do OpenSSL (k ALEATORIO, nao o
        // da RFC 6979), verificadas aqui.
        let chave = dir.join("chave.pem");
        let ok = |c: &mut Command| assert!(c.output().unwrap().status.success(), "{c:?}");
        ok(Command::new("openssl")
            .args([
                "ecparam",
                "-name",
                "prime256v1",
                "-genkey",
                "-noout",
                "-out",
            ])
            .arg(&chave));
        ok(Command::new("openssl")
            .args(["dgst", "-sha256", "-sign"])
            .arg(&chave)
            .arg("-out")
            .arg(dir.join("sig2"))
            .arg(dir.join("msg")));
        ok(Command::new("openssl")
            .args(["ec", "-pubout", "-outform", "DER", "-in"])
            .arg(&chave)
            .arg("-out")
            .arg(dir.join("pub2.der")));
        let spki2 = std::fs::read(dir.join("pub2.der")).unwrap();
        let q2 = &spki2[spki2.len() - 65..];
        let (r2, s2) = r_s_do_der(&std::fs::read(dir.join("sig2")).unwrap());
        assert!(
            verificar(q2, msg, &r2, &s2),
            "esta casa recusou a assinatura do OpenSSL"
        );
        let mut s_errado = s2;
        s_errado[0] ^= 0x40;
        assert!(!verificar(q2, msg, &r2, &s_errado));
    }

    /// O diretorio do teste do OpenSSL, apagado no `Drop` -- inclusive quando
    /// uma assercao no meio derruba o teste. O `phxsql-core` nao tem o
    /// `apoio_teste` do store e do servidor, e este e o unico teste dele que
    /// precisa de arquivo.
    struct DirDoTeste(std::path::PathBuf);

    impl DirDoTeste {
        fn novo() -> DirDoTeste {
            let d = std::env::temp_dir().join(format!("phx-p256-{}", std::process::id()));
            std::fs::create_dir_all(&d).unwrap();
            DirDoTeste(d)
        }
    }

    impl Drop for DirDoTeste {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn r_s_do_der(der: &[u8]) -> ([u8; 32], [u8; 32]) {
        let (seq, _) = crate::asn1::analisar(der).unwrap();
        let itens = crate::asn1::filhos(seq.conteudo).unwrap();
        let fixo = |e: &crate::asn1::Elemento<'_>| {
            let m = crate::asn1::decodificar_inteiro(e.conteudo).unwrap();
            let mut o = [0u8; 32];
            o[32 - m.len()..].copy_from_slice(&m);
            o
        };
        (fixo(&itens[0]), fixo(&itens[1]))
    }

    fn hex_para_vec(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    fn der_da_assinatura(r: &[u8; 32], s: &[u8; 32]) -> Vec<u8> {
        crate::asn1::sequencia(&[crate::asn1::inteiro(r), crate::asn1::inteiro(s)])
    }
}
