//! ECDSA P-384 com SHA-384 -- so VERIFICA (pedido 572, T6c-1).
//!
//! Entra porque `ecdsa_secp384r1_sha384` e o que boa parte das autoridades
//! usa para assinar intermediarias e folhas, e o cliente TLS que fala com
//! terceiros precisa conferir isso. Assinar com P-384 nao entra: as
//! identidades desta casa sao P-256 (`p256.rs`, de sequencia fixa).
//!
//! A aritmetica e a do [`crate::bigint`] -- dado publico, sem tempo
//! constante, e por isso aqui ha desvio a vontade (ponto no infinito, `H = 0`
//! na soma). Coordenadas jacobianas com `a = -3`: dobra `dbl-2001-b` e soma
//! `add-2007-bl` da Explicit-Formulas Database, com os casos especiais
//! tratados a mao, que e onde os vetores do Wycheproof batem.
//!
//! Os parametros da curva (`p`, `b`, `G`, `n`) vieram do `openssl ecparam
//! -name secp384r1 -param_enc explicit -text`, e o que os prova e o conjunto
//! de vetores: CAVP `SigVer` e Wycheproof nao passariam com uma constante
//! errada.

use crate::bigint::{self, Modulo};
use std::sync::OnceLock;

const P: &str = "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff0000000000000000ffffffff";
const B: &str = "b3312fa7e23ee7e4988e056be3f82d19181d9c6efe8141120314088f5013875ac656398d8a2ed19d2a85c8edd3ec2aef";
const GX: &str = "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7";
const GY: &str = "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c00a60b1ce1d7e819d7a431d7c90ea0e5f";
const N: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf581a0db248b0a77aecec196accc52973";

/// Tamanho de uma coordenada e de um escalar, em bytes.
pub const TAM: usize = 48;

struct Curva {
    p: Modulo,
    n: Modulo,
    /// `b` e `3`, ja em forma de Montgomery do corpo.
    b: Vec<u64>,
    tres: Vec<u64>,
    g: Ponto,
}

fn hx(t: &str) -> Vec<u8> {
    crate::hash::de_hex(t).expect("constante da P-384")
}

fn curva() -> &'static Curva {
    static C: OnceLock<Curva> = OnceLock::new();
    C.get_or_init(|| {
        let p = Modulo::novo(&hx(P)).expect("p impar");
        let n = Modulo::novo(&hx(N)).expect("n impar");
        let b = p.entrar(&bigint::de_be(&hx(B)));
        let tres = p.entrar(&[3]);
        let g = Ponto {
            x: p.entrar(&bigint::de_be(&hx(GX))),
            y: p.entrar(&bigint::de_be(&hx(GY))),
            z: p.um(),
        };
        Curva { p, n, b, tres, g }
    })
}

/// Ponto em coordenadas jacobianas, tudo em forma de Montgomery; `z = 0` e o
/// ponto no infinito.
#[derive(Clone)]
struct Ponto {
    x: Vec<u64>,
    y: Vec<u64>,
    z: Vec<u64>,
}

impl Ponto {
    fn infinito() -> Ponto {
        Ponto {
            x: vec![0; 6],
            y: vec![0; 6],
            z: vec![0; 6],
        }
    }

    fn eh_infinito(&self) -> bool {
        bigint::eh_zero(&self.z)
    }

    fn dobrar(&self) -> Ponto {
        let c = curva();
        let f = &c.p;
        if self.eh_infinito() || bigint::eh_zero(&self.y) {
            return Ponto::infinito();
        }
        let delta = f.mul(&self.z, &self.z);
        let gamma = f.mul(&self.y, &self.y);
        let beta = f.mul(&self.x, &gamma);
        let alfa = f.mul(
            &c.tres,
            &f.mul(&f.subtrair(&self.x, &delta), &f.somar(&self.x, &delta)),
        );
        let beta4 = f.somar(&f.somar(&beta, &beta), &f.somar(&beta, &beta));
        let beta8 = f.somar(&beta4, &beta4);
        let x3 = f.subtrair(&f.mul(&alfa, &alfa), &beta8);
        let yz = f.somar(&self.y, &self.z);
        let z3 = f.subtrair(&f.subtrair(&f.mul(&yz, &yz), &gamma), &delta);
        let g2 = f.mul(&gamma, &gamma);
        let g4 = f.somar(&g2, &g2);
        let g8 = f.somar(&f.somar(&g4, &g4), &f.somar(&g4, &g4));
        let y3 = f.subtrair(&f.mul(&alfa, &f.subtrair(&beta4, &x3)), &g8);
        Ponto {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    fn somar(&self, o: &Ponto) -> Ponto {
        if self.eh_infinito() {
            return o.clone();
        }
        if o.eh_infinito() {
            return self.clone();
        }
        let f = &curva().p;
        let z1z1 = f.mul(&self.z, &self.z);
        let z2z2 = f.mul(&o.z, &o.z);
        let u1 = f.mul(&self.x, &z2z2);
        let u2 = f.mul(&o.x, &z1z1);
        let s1 = f.mul(&f.mul(&self.y, &o.z), &z2z2);
        let s2 = f.mul(&f.mul(&o.y, &self.z), &z1z1);
        let h = f.subtrair(&u2, &u1);
        let r0 = f.subtrair(&s2, &s1);
        if bigint::eh_zero(&h) {
            // Mesmo x: ou e o mesmo ponto (dobra), ou o oposto (infinito).
            return if bigint::eh_zero(&r0) {
                self.dobrar()
            } else {
                Ponto::infinito()
            };
        }
        let h2 = f.somar(&h, &h);
        let i = f.mul(&h2, &h2);
        let j = f.mul(&h, &i);
        let r = f.somar(&r0, &r0);
        let v = f.mul(&u1, &i);
        let x3 = f.subtrair(&f.subtrair(&f.mul(&r, &r), &j), &f.somar(&v, &v));
        let s1j = f.mul(&s1, &j);
        let y3 = f.subtrair(&f.mul(&r, &f.subtrair(&v, &x3)), &f.somar(&s1j, &s1j));
        let zz = f.somar(&self.z, &o.z);
        let z3 = f.mul(&f.subtrair(&f.subtrair(&f.mul(&zz, &zz), &z1z1), &z2z2), &h);
        Ponto {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    /// O `x` afim, fora da forma de Montgomery.
    fn x_afim(&self) -> Vec<u64> {
        let f = &curva().p;
        let zi = f.inverso_primo(&self.z);
        f.sair(&f.mul(&self.x, &f.mul(&zi, &zi)))
    }
}

/// Le `04 || X || Y`, conferindo que as coordenadas sao menores que `p` e que
/// o ponto esta na curva -- sem isso, a verificacao aceitaria ponto de outra
/// curva, que e o ataque de curva invalida.
fn ponto_publico(publica: &[u8]) -> Option<Ponto> {
    if publica.len() != 1 + 2 * TAM || publica[0] != 0x04 {
        return None;
    }
    let c = curva();
    let f = &c.p;
    let x = f.ajustar(&bigint::de_be(&publica[1..1 + TAM]))?;
    let y = f.ajustar(&bigint::de_be(&publica[1 + TAM..]))?;
    let (x, y) = (f.entrar(&x), f.entrar(&y));
    // y^2 = x^3 - 3x + b
    let x3 = f.mul(&f.mul(&x, &x), &x);
    let tres_x = f.mul(&c.tres, &x);
    let direita = f.somar(&f.subtrair(&x3, &tres_x), &c.b);
    if f.mul(&y, &y) != direita {
        return None;
    }
    Some(Ponto { x, y, z: f.um() })
}

/// `u1*G + u2*Q` pelo truque de Shamir: uma passada de dobras para os dois.
fn combinar(u1: &[u8], q: &Ponto, u2: &[u8]) -> Ponto {
    let g = &curva().g;
    let gq = g.somar(q);
    let mut r = Ponto::infinito();
    for (a, b) in u1.iter().zip(u2) {
        for i in (0..8).rev() {
            r = r.dobrar();
            match ((a >> i) & 1, (b >> i) & 1) {
                (1, 1) => r = r.somar(&gq),
                (1, 0) => r = r.somar(g),
                (0, 1) => r = r.somar(q),
                _ => {}
            }
        }
    }
    r
}

/// Verifica `(r, s)` sobre o RESUMO ja calculado (SEC 1 §4.1.4). O resumo
/// mais longo que 384 bits e cortado nos 48 bytes de cima, como a norma manda
/// -- e o caso da P-384 com SHA-512.
pub fn verificar_resumo(publica: &[u8], resumo: &[u8], r: &[u8], s: &[u8]) -> bool {
    let c = curva();
    let Some(q) = ponto_publico(publica) else {
        return false;
    };
    let n = &c.n;
    let (Some(r), Some(s)) = (n.ajustar(&bigint::de_be(r)), n.ajustar(&bigint::de_be(s))) else {
        return false;
    };
    if bigint::eh_zero(&r) || bigint::eh_zero(&s) {
        return false;
    }
    let e = bigint::de_be(&resumo[..resumo.len().min(TAM)]);
    // Resumo menor que 48 bytes: o inteiro e ele mesmo; com 48, pode passar
    // de `n` e se reduz.
    let e = n.reduzir(&e);
    let w = n.inverso_primo(&n.entrar(&s));
    let u1 = n.sair(&n.mul(&n.entrar(&e), &w));
    let u2 = n.sair(&n.mul(&n.entrar(&r), &w));
    let (Some(u1), Some(u2)) = (bigint::para_be(&u1, TAM), bigint::para_be(&u2, TAM)) else {
        return false;
    };
    let x = combinar(&u1, &q, &u2);
    if x.eh_infinito() {
        return false;
    }
    // `v = x mod n`; `x < p` e `p < 2n`, entao basta reduzir.
    n.reduzir(&x.x_afim()) == r
}

/// Verifica sobre a mensagem, com SHA-384.
pub fn verificar(publica: &[u8], mensagem: &[u8], r: &[u8], s: &[u8]) -> bool {
    verificar_resumo(publica, &crate::sha512::sha384(mensagem), r, s)
}

/// `(r, s)` de um `ECDSA-Sig-Value` em DER (RFC 3279 §2.2.3): SEQUENCE de
/// dois INTEGER positivos, minimos, sem sobra, cada um com no maximo 48
/// bytes. Estrito de proposito: o Wycheproof traz dezenas de codificacoes
/// tortas que uma leitura frouxa aceitaria, e cada uma vira uma segunda
/// assinatura valida para a mesma mensagem.
pub fn assinatura_de_der(der: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    use crate::asn1;
    let (seq, resto) = asn1::esperar(der, asn1::TAG_SEQUENCE).ok()?;
    if !resto.is_empty() {
        return None;
    }
    let partes = asn1::filhos(seq.conteudo).ok()?;
    let [r, s] = partes.as_slice() else {
        return None;
    };
    let mut saida = Vec::new();
    for el in [r, s] {
        if el.tag != asn1::TAG_INTEGER {
            return None;
        }
        let m = asn1::decodificar_inteiro(el.conteudo).ok()?;
        if m.len() > TAM {
            return None;
        }
        saida.push(m);
    }
    let s = saida.pop()?;
    let r = saida.pop()?;
    Some((r, s))
}

#[cfg(test)]
mod testes;
