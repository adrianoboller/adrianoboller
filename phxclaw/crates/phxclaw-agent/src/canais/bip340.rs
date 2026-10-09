//! Assinatura Schnorr sobre secp256k1 (BIP-340), escrita aqui para o Nostr: todo evento
//! do Nostr e assinado assim, e conferir a assinatura e o que faz a lista de chaves
//! permitidas valer -- sem ela, qualquer um poria a chave publica do dono num evento.
//!
//! Escrita a partir da norma (BIP-340 e SEC 2 para a curva), sem crate, e conferida contra
//! os 19 vetores oficiais do BIP-340 (`tests/dados/nostr/bip340-test-vectors.csv`). Serve
//! tambem o ECDH do NIP-44 (`ecdh_x`), e ai a chave secreta do bot multiplica um ponto que
//! QUALQUER um escolhe (a chave efemera do gift wrap).
//!
//! Por isso, sem desvio por segredo no FONTE: a multiplicacao e a escada sempre-soma com
//! a soma completa (sem caso especial), e as reducoes escolhem por mascara em vez de `if`.
//! O que isto NAO prova: o binario. Nenhuma medicao de tempo foi feita, e o compilador
//! pode, em tese, reintroduzir um desvio; o expoente de `pot_p` e publico de proposito.
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

/// Tudo-um quando `b`, tudo-zero quando nao. Com ela a escolha entre dois resultados ja
/// calculados nao vira desvio: um `if` sobre um bit da chave dizia, pelo tempo, qual era.
fn mascara(b: bool) -> u64 {
    (b as u64).wrapping_neg()
}

/// `a` onde a mascara e tudo-um, `b` onde e tudo-zero.
fn escolher(m: u64, a: &U, b: &U) -> U {
    let mut r = [0u64; 4];
    for i in 0..4 {
        r[i] = (a[i] & m) | (b[i] & !m);
    }
    r
}

/// (a + b) mod m, com a e b ja menores que m. As duas contas sempre acontecem e a mascara
/// escolhe: a subtracao condicional e o lugar classico por onde o tempo conta o segredo.
fn add_mod(a: &U, b: &U, m: &U) -> U {
    let (s, vai) = somar(a, b);
    let (d, emprestou) = subtrair(&s, m);
    escolher(mascara(vai | !emprestou), &d, &s)
}

fn sub_mod(a: &U, b: &U, m: &U) -> U {
    let (d, emprestou) = subtrair(a, b);
    let (e, _) = somar(&d, m);
    escolher(mascara(emprestou), &e, &d)
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

/// t mod p, por 2^256 = C (mod p): dobra a metade alta duas vezes. O terceiro vai-um (0 ou
/// 1) entra multiplicado em vez de testado, e a subtracao final e escolhida por mascara.
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
    // Com vai-um, y ficou menor que 2^67: somar C de novo nao transborda.
    let mut c = c * C as u128;
    for yi in y.iter_mut() {
        let v = *yi as u128 + c;
        *yi = v as u64;
        c = v >> 64;
    }
    let (d, emprestou) = subtrair(&y, &P);
    escolher(mascara(!emprestou), &d, &y)
}

fn mul_p(a: &U, b: &U) -> U {
    reduzir_p(produto(a, b))
}

/// t mod m bit a bit, do mais alto para o mais baixo. O bit entra como parcela (0 ou 1),
/// nunca como `if`: aqui passam o produto com a chave secreta e o nonce da assinatura.
fn reduzir(t: &[u64], m: &U) -> U {
    let mut r = [0u64; 4];
    for i in (0..t.len() * 64).rev() {
        r = add_mod(&r, &r, m);
        r = add_mod(&r, &[(t[i / 64] >> (i % 64)) & 1, 0, 0, 0], m);
    }
    r
}

fn mul_n(a: &U, b: &U) -> U {
    reduzir(&produto(a, b), &N)
}

/// Potencia com expoente PUBLICO (p - 2, (p + 1) / 4): o desvio pelo bit nao conta nada.
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

/// Ponto em coordenadas projetivas homogeneas (X : Y : Z), x = X/Z e y = Y/Z; o infinito e
/// (0 : 1 : 0). Homogeneas, e nao jacobianas, porque e nelas que existe a soma COMPLETA.
#[derive(Clone, Copy)]
struct Ponto(U, U, U);

const INFINITO: Ponto = Ponto([0; 4], [1, 0, 0, 0], [0; 4]);

/// 3b, com b = 7 na curva y^2 = x^3 + 7.
const B3: U = [21, 0, 0, 0];

/// Soma completa de Renes, Costello e Batina (2016, algoritmo 7, a = 0): a MESMA conta
/// serve para somar, dobrar e somar com o infinito, sem nenhum caso especial. Os casos
/// especiais (`if u1 == u2`, `if zero(z)`) eram desvios sobre o ponto intermediario da
/// multiplicacao pela chave secreta.
fn somar_pontos(a: &Ponto, b: &Ponto) -> Ponto {
    let mais = |x: &U, y: &U| add_mod(x, y, &P);
    let menos = |x: &U, y: &U| sub_mod(x, y, &P);
    let xx = mul_p(&a.0, &b.0);
    let yy = mul_p(&a.1, &b.1);
    let zz = mul_p(&a.2, &b.2);
    let xy = menos(
        &mul_p(&mais(&a.0, &a.1), &mais(&b.0, &b.1)),
        &mais(&xx, &yy),
    );
    let yz = menos(
        &mul_p(&mais(&a.1, &a.2), &mais(&b.1, &b.2)),
        &mais(&yy, &zz),
    );
    let xz = menos(
        &mul_p(&mais(&a.0, &a.2), &mais(&b.0, &b.2)),
        &mais(&xx, &zz),
    );
    let bzz3 = mul_p(&B3, &zz);
    let yy_m = menos(&yy, &bzz3);
    let yy_p = mais(&yy, &bzz3);
    let byz3 = mul_p(&B3, &yz);
    let xx3 = mais(&mais(&xx, &xx), &xx);
    let bxx9 = mul_p(&B3, &xx3);
    let x3 = menos(&mul_p(&xy, &yy_m), &mul_p(&byz3, &xz));
    let y3 = mais(&mul_p(&yy_p, &yy_m), &mul_p(&bxx9, &xz));
    let z3 = mais(&mul_p(&yz, &yy_p), &mul_p(&xx3, &xy));
    Ponto(x3, y3, z3)
}

/// k * P pela escada sempre-soma: a cada bit dobra E soma, e a mascara escolhe qual dos
/// dois fica. O numero de contas nao depende de k -- o de antes somava so nos bits 1, e o
/// tempo da multiplicacao contava quantos bits 1 a chave secreta tinha.
fn multiplicar(k: &U, p: &Ponto) -> Ponto {
    let mut r = INFINITO;
    for i in (0..256).rev() {
        r = somar_pontos(&r, &r);
        let t = somar_pontos(&r, p);
        let m = ((k[i / 64] >> (i % 64)) & 1).wrapping_neg();
        r = Ponto(
            escolher(m, &t.0, &r.0),
            escolher(m, &t.1, &r.1),
            escolher(m, &t.2, &r.2),
        );
    }
    r
}

/// (x, y) afim; None no infinito.
fn afim(p: &Ponto) -> Option<(U, U)> {
    if zero(&p.2) {
        return None;
    }
    let zi = inv_p(&p.2);
    Some((mul_p(&p.0, &zi), mul_p(&p.1, &zi)))
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

/// A chave secreta como escalar em [1, n-1].
fn escalar(segredo: &[u8; 32]) -> Result<U, String> {
    let d = de_bytes(segredo);
    if zero(&d) || maior_ou_igual(&d, &N) {
        return Err("chave secreta fora de [1, n-1]".into());
    }
    Ok(d)
}

/// Chave publica (so o x, 32 bytes) da chave secreta.
pub fn chave_publica(segredo: &[u8; 32]) -> Result<[u8; 32], String> {
    let (x, _) = afim(&multiplicar(&escalar(segredo)?, &g())).ok_or("ponto no infinito")?;
    Ok(em_bytes(&x))
}

/// O x (32 bytes, SEM hash) de `segredo * P`, com P o ponto de x = `pk` e y par: o
/// `secp256k1_ecdh` do NIP-44. A chave publica passa pelo `lift_x`, entao ponto fora da
/// curva -- inclusive os da torcao, que dariam um subgrupo pequeno e vazariam a chave
/// secreta aos pedacos -- volta erro antes de qualquer conta com o segredo.
pub fn ecdh_x(segredo: &[u8; 32], pk: &[u8; 32]) -> Result<[u8; 32], String> {
    let d = escalar(segredo)?;
    let p = levantar_x(&de_bytes(pk)).ok_or("chave publica fora da curva")?;
    let (x, _) = afim(&multiplicar(&d, &p)).ok_or("ponto no infinito")?;
    Ok(em_bytes(&x))
}

/// Assina `msg` (32 bytes, no Nostr o id do evento) com `aux` de 32 bytes.
pub fn assinar(segredo: &[u8; 32], msg: &[u8; 32], aux: &[u8; 32]) -> Result<[u8; 64], String> {
    assinar_msg(segredo, msg, aux)
}

/// O BIP-340 aceita mensagem de qualquer tamanho (vetores 15 a 18); o Nostr so usa 32.
fn assinar_msg(segredo: &[u8; 32], msg: &[u8], aux: &[u8; 32]) -> Result<[u8; 64], String> {
    let d0 = escalar(segredo)?;
    let (px, py) = afim(&multiplicar(&d0, &g())).ok_or("ponto no infinito")?;
    // Negar ou nao por mascara: a paridade de y e funcao do segredo.
    let d = escolher(mascara(par(&py)), &d0, &subtrair(&N, &d0).0);
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
    let k = escolher(mascara(par(&ry)), &k0, &subtrair(&N, &k0).0);
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

    /// Os 19 vetores do `test-vectors.csv` do BIP-340, copiado sem mexer do repositorio
    /// `bitcoin/bips` (o sha256 do arquivo e conferido, para ninguem «ajustar» um vetor).
    #[test]
    fn assinatura_contra_os_vetores_do_bip_340() {
        use sha2::{Digest, Sha256};
        let csv = include_str!("../../tests/dados/nostr/bip340-test-vectors.csv");
        assert_eq!(
            hex(&Sha256::digest(csv.as_bytes())),
            "34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d"
        );
        let mut n = 0;
        for linha in csv.lines().skip(1) {
            let c: Vec<&str> = linha.split(',').collect();
            let (sk, pk, aux, msg, sig, ok) = (c[1], c[2], c[3], c[4], c[5], c[6] == "TRUE");
            let msg = (0..msg.len() / 2)
                .map(|i| u8::from_str_radix(&msg[i * 2..i * 2 + 2], 16).unwrap())
                .collect::<Vec<u8>>();
            let sig: [u8; 64] = de_hex(sig).unwrap();
            if !sk.is_empty() {
                let sk = h32(sk);
                assert_eq!(hex(&chave_publica(&sk).unwrap()).to_uppercase(), pk);
                let s = assinar_msg(&sk, &msg, &h32(aux)).unwrap();
                assert_eq!(s, sig, "vetor {}", c[0]);
            }
            assert_eq!(
                conferir(&h32(pk), &msg, &sig),
                ok,
                "vetor {}: {}",
                c[0],
                c[7]
            );
            n += 1;
        }
        assert_eq!(n, 19);
    }

    /// A escada com a soma completa contra a soma repetida de G: 1G..40G pelos dois
    /// caminhos, e 3G contra o ponto conhecido.
    #[test]
    fn escada_bate_com_a_soma_repetida() {
        let mut acumulado = INFINITO;
        for k in 1u64..=40 {
            acumulado = somar_pontos(&acumulado, &g());
            let a = afim(&acumulado).unwrap();
            let b = afim(&multiplicar(&[k, 0, 0, 0], &g())).unwrap();
            assert!(a == b, "{k}G diverge");
        }
        // n - 1 vezes G e -G: mesmo x, y oposto; n vezes G e o infinito.
        let menos_um = subtrair(&N, &[1, 0, 0, 0]).0;
        let (x, y) = afim(&multiplicar(&menos_um, &g())).unwrap();
        assert!(x == GX && y == sub_mod(&[0; 4], &GY, &P));
        assert!(afim(&multiplicar(&N, &g())).is_none());
    }

    #[test]
    fn ecdh_e_simetrico_e_recusa_ponto_fora_da_curva() {
        let (a, b) = (h32(&"11".repeat(32)), h32(&"22".repeat(32)));
        let (pa, pb) = (chave_publica(&a).unwrap(), chave_publica(&b).unwrap());
        assert_eq!(ecdh_x(&a, &pb).unwrap(), ecdh_x(&b, &pa).unwrap());
        assert!(ecdh_x(&a, &[0xff; 32]).is_err());
        assert!(ecdh_x(&[0; 32], &pb).is_err());
    }
}
