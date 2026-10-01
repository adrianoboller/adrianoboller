//! Assinatura Schnorr sobre secp256k1 (BIP-340), escrita aqui para o Nostr: todo evento
//! do Nostr e assinado assim, e conferir a assinatura e o que faz a lista de chaves
//! permitidas valer -- sem ela, qualquer um poria a chave publica do dono num evento.
//!
//! Escrita a partir da norma (BIP-340 e SEC 2 para a curva), sem crate, e conferida contra
//! os vetores oficiais do BIP-340 nos testes. Nao e tempo constante: assina com a chave do
//! bot num processo que so ela usa, e o canal conta como PARCIAL tambem por isso.
//!
//! Inteiros de 256 bits em quatro palavras de 64 (a menos significativa primeiro). A
//! reducao modulo p usa p = 2^256 - 0x1000003D1; a modulo n (poucas contas por assinatura)
//! e a divisao bit a bit, mais lenta e mais simples de conferir.

use sha2::{Digest, Sha256};

type U = [u64; 4];

const P: U = [
    0xFFFFFFFEFFFFFC2F,
    0xFFFFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFFF,
];
const N: U = [
    0xBFD25E8CD0364141,
    0xBAAEDCE6AF48A03B,
    0xFFFFFFFFFFFFFFFE,
    0xFFFFFFFFFFFFFFFF,
];
const GX: U = [
    0x59F2815B16F81798,
    0x029BFCDB2DCE28D9,
    0x55A06295CE870B07,
    0x79BE667EF9DCBBAC,
];
const GY: U = [
    0x9C47D08FFB10D4B8,
    0xFD17B448A6855419,
    0x5DA4FBFC0E1108A8,
    0x483ADA7726A3C465,
];
const C: u64 = 0x1000003D1;

fn de_bytes(b: &[u8]) -> U {
    let mut u = [0u64; 4];
    for (i, c) in b.chunks(8).enumerate() {
        u[3 - i] = u64::from_be_bytes(c.try_into().expect("8 bytes"));
    }
    u
}

fn em_bytes(u: &U) -> [u8; 32] {
    let mut b = [0u8; 32];
    for i in 0..4 {
        b[i * 8..i * 8 + 8].copy_from_slice(&u[3 - i].to_be_bytes());
    }
    b
}

fn maior_ou_igual(a: &U, b: &U) -> bool {
    for i in (0..4).rev() {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

fn zero(a: &U) -> bool {
    a.iter().all(|x| *x == 0)
}

fn somar(a: &U, b: &U) -> (U, bool) {
    let mut r = [0u64; 4];
    let mut c = 0u128;
    for i in 0..4 {
        let v = a[i] as u128 + b[i] as u128 + c;
        r[i] = v as u64;
        c = v >> 64;
    }
    (r, c != 0)
}

fn subtrair(a: &U, b: &U) -> (U, bool) {
    let mut r = [0u64; 4];
    let mut emprestimo = false;
    for i in 0..4 {
        let (v, b1) = a[i].overflowing_sub(b[i]);
        let (v, b2) = v.overflowing_sub(emprestimo as u64);
        r[i] = v;
        emprestimo = b1 || b2;
    }
    (r, emprestimo)
}

/// (a + b) mod m, com a e b ja menores que m.
fn add_mod(a: &U, b: &U, m: &U) -> U {
    let (s, vai) = somar(a, b);
    if vai || maior_ou_igual(&s, m) {
        subtrair(&s, m).0
    } else {
        s
    }
}

fn sub_mod(a: &U, b: &U, m: &U) -> U {
    let (d, emprestou) = subtrair(a, b);
    if emprestou { somar(&d, m).0 } else { d }
}

fn produto(a: &U, b: &U) -> [u64; 8] {
    let mut t = [0u64; 8];
    for i in 0..4 {
        let mut c = 0u128;
        for j in 0..4 {
            let v = t[i + j] as u128 + a[i] as u128 * b[j] as u128 + c;
            t[i + j] = v as u64;
            c = v >> 64;
        }
        t[i + 4] = c as u64;
    }
    t
}

/// t mod p, por 2^256 = C (mod p): dobra a metade alta duas vezes.
fn reduzir_p(t: [u64; 8]) -> U {
    let mut x = [0u64; 5];
    let mut c = 0u128;
    for i in 0..4 {
        let v = t[i] as u128 + t[i + 4] as u128 * C as u128 + c;
        x[i] = v as u64;
        c = v >> 64;
    }
    x[4] = c as u64;
    let mut y = [0u64; 4];
    let mut c = x[4] as u128 * C as u128;
    for i in 0..4 {
        let v = x[i] as u128 + c;
        y[i] = v as u64;
        c = v >> 64;
    }
    if c != 0 {
        let mut c = C as u128;
        for yi in y.iter_mut() {
            let v = *yi as u128 + c;
            *yi = v as u64;
            c = v >> 64;
        }
    }
    if maior_ou_igual(&y, &P) {
        y = subtrair(&y, &P).0;
    }
    y
}

fn mul_p(a: &U, b: &U) -> U {
    reduzir_p(produto(a, b))
}

/// t mod m bit a bit, do mais alto para o mais baixo.
fn reduzir(t: &[u64], m: &U) -> U {
    let mut r = [0u64; 4];
    for i in (0..t.len() * 64).rev() {
        r = add_mod(&r, &r, m);
        if (t[i / 64] >> (i % 64)) & 1 == 1 {
            r = add_mod(&r, &[1, 0, 0, 0], m);
        }
    }
    r
}

fn mul_n(a: &U, b: &U) -> U {
    reduzir(&produto(a, b), &N)
}

fn pot_p(a: &U, e: &U) -> U {
    let mut r: U = [1, 0, 0, 0];
    for i in (0..256).rev() {
        r = mul_p(&r, &r);
        if (e[i / 64] >> (i % 64)) & 1 == 1 {
            r = mul_p(&r, a);
        }
    }
    r
}

fn inv_p(a: &U) -> U {
    pot_p(a, &subtrair(&P, &[2, 0, 0, 0]).0)
}

/// Ponto em coordenadas jacobianas (X, Y, Z); Z = 0 e o ponto no infinito.
#[derive(Clone, Copy)]
struct Ponto(U, U, U);

const INFINITO: Ponto = Ponto([0; 4], [1, 0, 0, 0], [0; 4]);

fn dobrar(p: &Ponto) -> Ponto {
    let Ponto(x, y, z) = p;
    if zero(z) || zero(y) {
        return INFINITO;
    }
    let y2 = mul_p(y, y);
    let s = mul_p(&mul_p(x, &y2), &[4, 0, 0, 0]);
    let m = mul_p(&mul_p(x, x), &[3, 0, 0, 0]);
    let x3 = sub_mod(&mul_p(&m, &m), &add_mod(&s, &s, &P), &P);
    let y4 = mul_p(&y2, &y2);
    let y3 = sub_mod(
        &mul_p(&m, &sub_mod(&s, &x3, &P)),
        &mul_p(&y4, &[8, 0, 0, 0]),
        &P,
    );
    let z3 = mul_p(&add_mod(y, y, &P), z);
    Ponto(x3, y3, z3)
}

fn somar_pontos(a: &Ponto, b: &Ponto) -> Ponto {
    if zero(&a.2) {
        return *b;
    }
    if zero(&b.2) {
        return *a;
    }
    let z1z1 = mul_p(&a.2, &a.2);
    let z2z2 = mul_p(&b.2, &b.2);
    let u1 = mul_p(&a.0, &z2z2);
    let u2 = mul_p(&b.0, &z1z1);
    let s1 = mul_p(&a.1, &mul_p(&b.2, &z2z2));
    let s2 = mul_p(&b.1, &mul_p(&a.2, &z1z1));
    if u1 == u2 {
        return if s1 == s2 { dobrar(a) } else { INFINITO };
    }
    let h = sub_mod(&u2, &u1, &P);
    let r = sub_mod(&s2, &s1, &P);
    let h2 = mul_p(&h, &h);
    let h3 = mul_p(&h2, &h);
    let u1h2 = mul_p(&u1, &h2);
    let x3 = sub_mod(
        &sub_mod(&mul_p(&r, &r), &h3, &P),
        &add_mod(&u1h2, &u1h2, &P),
        &P,
    );
    let y3 = sub_mod(&mul_p(&r, &sub_mod(&u1h2, &x3, &P)), &mul_p(&s1, &h3), &P);
    let z3 = mul_p(&h, &mul_p(&a.2, &b.2));
    Ponto(x3, y3, z3)
}

fn multiplicar(k: &U, p: &Ponto) -> Ponto {
    let mut r = INFINITO;
    for i in (0..256).rev() {
        r = dobrar(&r);
        if (k[i / 64] >> (i % 64)) & 1 == 1 {
            r = somar_pontos(&r, p);
        }
    }
    r
}

/// (x, y) afim; None no infinito.
fn afim(p: &Ponto) -> Option<(U, U)> {
    if zero(&p.2) {
        return None;
    }
    let zi = inv_p(&p.2);
    let zi2 = mul_p(&zi, &zi);
    Some((mul_p(&p.0, &zi2), mul_p(&p.1, &mul_p(&zi2, &zi))))
}

fn g() -> Ponto {
    Ponto(GX, GY, [1, 0, 0, 0])
}

fn par(y: &U) -> bool {
    y[0] & 1 == 0
}

fn hash_marcado(marca: &str, partes: &[&[u8]]) -> [u8; 32] {
    let t = Sha256::digest(marca.as_bytes());
    let mut h = Sha256::new();
    h.update(t);
    h.update(t);
    for p in partes {
        h.update(p);
    }
    h.finalize().into()
}

/// O ponto de x dado com y par (`lift_x` do BIP-340).
fn levantar_x(x: &U) -> Option<Ponto> {
    if maior_ou_igual(x, &P) {
        return None;
    }
    let c = add_mod(&mul_p(&mul_p(x, x), x), &[7, 0, 0, 0], &P);
    // (p + 1) / 4
    let e = {
        let (s, _) = somar(&P, &[1, 0, 0, 0]);
        [
            (s[0] >> 2) | (s[1] << 62),
            (s[1] >> 2) | (s[2] << 62),
            (s[2] >> 2) | (s[3] << 62),
            s[3] >> 2,
        ]
    };
    let y = pot_p(&c, &e);
    if mul_p(&y, &y) != c {
        return None;
    }
    let y = if par(&y) { y } else { sub_mod(&[0; 4], &y, &P) };
    Some(Ponto(*x, y, [1, 0, 0, 0]))
}

/// Chave publica (so o x, 32 bytes) da chave secreta.
pub fn chave_publica(segredo: &[u8; 32]) -> Result<[u8; 32], String> {
    let d = de_bytes(segredo);
    if zero(&d) || maior_ou_igual(&d, &N) {
        return Err("chave secreta fora de [1, n-1]".into());
    }
    let (x, _) = afim(&multiplicar(&d, &g())).ok_or("ponto no infinito")?;
    Ok(em_bytes(&x))
}

/// Assina `msg` (32 bytes, no Nostr o id do evento) com `aux` de 32 bytes.
pub fn assinar(segredo: &[u8; 32], msg: &[u8; 32], aux: &[u8; 32]) -> Result<[u8; 64], String> {
    let d0 = de_bytes(segredo);
    if zero(&d0) || maior_ou_igual(&d0, &N) {
        return Err("chave secreta fora de [1, n-1]".into());
    }
    let (px, py) = afim(&multiplicar(&d0, &g())).ok_or("ponto no infinito")?;
    let d = if par(&py) { d0 } else { subtrair(&N, &d0).0 };
    let a = hash_marcado("BIP0340/aux", &[aux]);
    let db = em_bytes(&d);
    let t: Vec<u8> = db.iter().zip(a.iter()).map(|(x, y)| x ^ y).collect();
    let pxb = em_bytes(&px);
    let k0 = reduzir(
        &de_bytes(&hash_marcado("BIP0340/nonce", &[&t, &pxb, msg])),
        &N,
    );
    if zero(&k0) {
        return Err("nonce zero".into());
    }
    let (rx, ry) = afim(&multiplicar(&k0, &g())).ok_or("ponto no infinito")?;
    let k = if par(&ry) { k0 } else { subtrair(&N, &k0).0 };
    let rxb = em_bytes(&rx);
    let e = reduzir(
        &de_bytes(&hash_marcado("BIP0340/challenge", &[&rxb, &pxb, msg])),
        &N,
    );
    let s = add_mod(&k, &mul_n(&e, &d), &N);
    let mut sig = [0u8; 64];
    sig[..32].copy_from_slice(&rxb);
    sig[32..].copy_from_slice(&em_bytes(&s));
    Ok(sig)
}

/// Confere a assinatura de `msg` pela chave publica `pk` (x de 32 bytes).
pub fn conferir(pk: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
    let Some(p) = levantar_x(&de_bytes(pk)) else {
        return false;
    };
    let r = de_bytes(&sig[..32]);
    let s = de_bytes(&sig[32..]);
    if maior_ou_igual(&r, &P) || maior_ou_igual(&s, &N) {
        return false;
    }
    let e = reduzir(
        &de_bytes(&hash_marcado("BIP0340/challenge", &[&sig[..32], pk, msg])),
        &N,
    );
    let menos_e = sub_mod(&[0; 4], &e, &N);
    let rr = somar_pontos(&multiplicar(&s, &g()), &multiplicar(&menos_e, &p));
    match afim(&rr) {
        Some((x, y)) => par(&y) && x == r,
        None => false,
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn de_hex<const T: usize>(s: &str) -> Result<[u8; T], String> {
    let s = s.trim();
    if s.len() != T * 2 {
        return Err(format!("esperava {} caracteres hexadecimais", T * 2));
    }
    let mut b = [0u8; T];
    for (i, x) in b.iter_mut().enumerate() {
        *x = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h32(s: &str) -> [u8; 32] {
        de_hex(s).unwrap()
    }

    #[test]
    fn tres_g_e_o_ponto_conhecido() {
        let mut d = [0u8; 32];
        d[31] = 3;
        assert_eq!(
            hex(&chave_publica(&d).unwrap()).to_uppercase(),
            "F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9"
        );
    }

    /// Vetores 0 e 1 do `test-vectors.csv` do BIP-340.
    #[test]
    fn assinatura_contra_os_vetores_do_bip_340() {
        let casos = [
            (
                "0000000000000000000000000000000000000000000000000000000000000003",
                "F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "E907831F80848D1069A5371B402410364BDF1C5F8307B0084C55F1CE2DCA821525F66A4A85EA8B71E482A74F382D2CE5EBEEE8FDB2172F477DF4900D310536C0",
            ),
            (
                "B7E151628AED2A6ABF7158809CF4F3C762E7160F38B4DA56A784D9045190CFEF",
                "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
                "0000000000000000000000000000000000000000000000000000000000000001",
                "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
                "6896BD60EEAE296DB48A229FF71DFE071BDE413E6D43F917DC8DCF8C78DE33418906D11AC976ABCCB20B091292BFF4EA897EFCB639EA871CFA95F6DE339E4B0A",
            ),
        ];
        for (sk, pk, aux, msg, sig) in casos {
            let (sk, aux, msg) = (h32(sk), h32(aux), h32(msg));
            assert_eq!(hex(&chave_publica(&sk).unwrap()).to_uppercase(), pk);
            let s = assinar(&sk, &msg, &aux).unwrap();
            assert_eq!(hex(&s).to_uppercase(), sig);
            assert!(conferir(&h32(pk), &msg, &s));
            let mut errada = s;
            errada[63] ^= 1;
            assert!(
                !conferir(&h32(pk), &msg, &errada),
                "assinatura mexida passou"
            );
            let mut outra = msg;
            outra[0] ^= 1;
            assert!(!conferir(&h32(pk), &outra, &s), "mensagem mexida passou");
        }
    }
}
