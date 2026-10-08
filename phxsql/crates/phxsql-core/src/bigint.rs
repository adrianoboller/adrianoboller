//! Inteiros grandes para dado PUBLICO -- a verificacao de RSA e de ECDSA
//! P-384 (pedido 572, T6c-1).
//!
//! # Por que «publico» esta no nome do contrato
//!
//! Nada aqui e de tempo constante: a exponenciacao desvia pelo bit do
//! expoente, a reducao final desvia pela comparacao. Isso e certo para o que
//! este modulo faz -- conferir uma assinatura, em que assinatura, chave
//! publica e mensagem sao todas conhecidas de quem escuta o fio -- e errado
//! para qualquer segredo. Por isso o modulo nao tem `assinar`, nao tem chave
//! privada, e o P-256 (que assina) continua com a aritmetica propria dele, de
//! sequencia fixa.
//!
//! Montgomery (CIOS) para todo modulo impar, que e o caso de todo modulo RSA
//! e de todo primo de curva; `R^2 mod m` sai de dobrar 1 ate la, sem
//! constante digitada.

use std::cmp::Ordering;

/// Big-endian para palavras de 64 bits, a menos significativa primeiro.
pub fn de_be(b: &[u8]) -> Vec<u64> {
    let mut l = vec![0u64; b.len().div_ceil(8).max(1)];
    for (i, byte) in b.iter().rev().enumerate() {
        l[i / 8] |= (*byte as u64) << (8 * (i % 8));
    }
    l
}

/// Palavras para big-endian em `tam` bytes; `None` quando o numero nao cabe.
pub fn para_be(l: &[u64], tam: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; tam];
    for i in 0..l.len() * 8 {
        let byte = (l[i / 8] >> (8 * (i % 8))) as u8;
        if i < tam {
            out[tam - 1 - i] = byte;
        } else if byte != 0 {
            return None;
        }
    }
    Some(out)
}

/// Compara dois naturais de tamanhos quaisquer.
pub fn comparar(a: &[u64], b: &[u64]) -> Ordering {
    for i in (0..a.len().max(b.len())).rev() {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

pub fn eh_zero(a: &[u64]) -> bool {
    a.iter().all(|&x| x == 0)
}

/// Quantos bits o natural ocupa.
pub fn bits(a: &[u64]) -> usize {
    for i in (0..a.len()).rev() {
        if a[i] != 0 {
            return 64 * i + 64 - a[i].leading_zeros() as usize;
        }
    }
    0
}

fn somar_n(a: &[u64], b: &[u64]) -> (Vec<u64>, u64) {
    let mut r = vec![0u64; a.len()];
    let mut c = 0u64;
    for i in 0..a.len() {
        let s = a[i] as u128 + b[i] as u128 + c as u128;
        r[i] = s as u64;
        c = (s >> 64) as u64;
    }
    (r, c)
}

fn subtrair_n(a: &[u64], b: &[u64]) -> (Vec<u64>, u64) {
    let mut r = vec![0u64; a.len()];
    let mut e = 0u64;
    for i in 0..a.len() {
        let (d1, e1) = a[i].overflowing_sub(b[i]);
        let (d2, e2) = d1.overflowing_sub(e);
        r[i] = d2;
        e = (e1 | e2) as u64;
    }
    (r, e)
}

/// Os inteiros modulo um `m` impar, em forma de Montgomery (`R = 2^(64n)`).
#[derive(Clone, Debug)]
pub struct Modulo {
    m: Vec<u64>,
    /// `-m^-1 mod 2^64`.
    m0: u64,
    /// `R^2 mod m`.
    r2: Vec<u64>,
}

impl Modulo {
    /// `None` quando `m` e par ou menor que 3: Montgomery pede `m` impar, e
    /// um modulo RSA par nao e modulo RSA.
    pub fn novo(m_be: &[u8]) -> Option<Modulo> {
        let mut m = de_be(m_be);
        while m.len() > 1 && m[m.len() - 1] == 0 {
            m.pop();
        }
        if m[0] & 1 == 0 || (m.len() == 1 && m[0] < 3) {
            return None;
        }
        let mut inv = 1u64;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(m[0].wrapping_mul(inv)));
        }
        let n = m.len();
        let mut c = Modulo {
            m,
            m0: inv.wrapping_neg(),
            r2: Vec::new(),
        };
        let mut x = vec![0u64; n];
        x[0] = 1;
        for _ in 0..128 * n {
            x = c.somar(&x, &x);
        }
        c.r2 = x;
        Some(c)
    }

    /// Palavras de cada elemento.
    pub fn limbs(&self) -> usize {
        self.m.len()
    }

    pub fn modulo(&self) -> &[u64] {
        &self.m
    }

    /// Bits do modulo -- o `modBits` da RFC 8017.
    pub fn bits(&self) -> usize {
        bits(&self.m)
    }

    /// Ajusta ao tamanho do modulo; `None` quando o numero e maior ou igual
    /// a `m` -- quem chama decide se isso e erro (assinatura RSA `s >= n`)
    /// ou se reduz (resumo do ECDSA).
    pub fn ajustar(&self, a: &[u64]) -> Option<Vec<u64>> {
        if comparar(a, &self.m) != Ordering::Less {
            return None;
        }
        let mut r = a.to_vec();
        r.resize(self.m.len(), 0);
        Some(r)
    }

    /// `(a + b) mod m`, com `a, b < m`.
    pub fn somar(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let (s, c) = somar_n(a, b);
        if c != 0 || comparar(&s, &self.m) != Ordering::Less {
            subtrair_n(&s, &self.m).0
        } else {
            s
        }
    }

    /// `(a - b) mod m`, com `a, b < m`.
    pub fn subtrair(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let (d, e) = subtrair_n(a, b);
        if e != 0 {
            somar_n(&d, &self.m).0
        } else {
            d
        }
    }

    /// Montgomery: `a * b * R^-1 mod m`. Vale para `a * b < m * R`, que cobre
    /// `a, b < m` e tambem `a < R` com `b < m` (o caso do `reduzir`).
    pub fn mul(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let n = self.m.len();
        let m = &self.m;
        let mut t = vec![0u64; n + 2];
        for &bi in b.iter().take(n) {
            let mut c = 0u128;
            for j in 0..n {
                let v = t[j] as u128 + a[j] as u128 * bi as u128 + c;
                t[j] = v as u64;
                c = v >> 64;
            }
            let v = t[n] as u128 + c;
            t[n] = v as u64;
            t[n + 1] = (v >> 64) as u64;

            let q = t[0].wrapping_mul(self.m0);
            let v = t[0] as u128 + q as u128 * m[0] as u128;
            let mut c = v >> 64;
            for j in 1..n {
                let v = t[j] as u128 + q as u128 * m[j] as u128 + c;
                t[j - 1] = v as u64;
                c = v >> 64;
            }
            let v = t[n] as u128 + c;
            t[n - 1] = v as u64;
            t[n] = t[n + 1] + (v >> 64) as u64;
        }
        let r = t[..n].to_vec();
        // O resultado e menor que 2m: uma subtracao basta, e o bit de cima
        // (`t[n]`) conta tambem.
        if t[n] != 0 || comparar(&r, m) != Ordering::Less {
            subtrair_n(&r, m).0
        } else {
            r
        }
    }

    /// Entra na forma de Montgomery; `a` pode ser qualquer natural de ate
    /// `n` palavras, inclusive maior que `m` -- sai reduzido.
    pub fn entrar(&self, a: &[u64]) -> Vec<u64> {
        let mut x = a.to_vec();
        x.resize(self.m.len(), 0);
        self.mul(&x, &self.r2)
    }

    pub fn sair(&self, a: &[u64]) -> Vec<u64> {
        let mut um = vec![0u64; self.m.len()];
        um[0] = 1;
        self.mul(a, &um)
    }

    /// `a mod m`, para `a` de ate `n` palavras.
    pub fn reduzir(&self, a: &[u64]) -> Vec<u64> {
        self.sair(&self.entrar(a))
    }

    /// O 1 em forma de Montgomery.
    pub fn um(&self) -> Vec<u64> {
        self.entrar(&[1])
    }

    /// `base^e`, com `base` em forma de Montgomery e o expoente em
    /// big-endian. Expoente publico: o desvio pelo bit e permitido aqui.
    pub fn potencia(&self, base: &[u64], e_be: &[u8]) -> Vec<u64> {
        let mut r = self.um();
        for byte in e_be {
            for i in (0..8).rev() {
                r = self.mul(&r, &r);
                if (byte >> i) & 1 == 1 {
                    r = self.mul(&r, base);
                }
            }
        }
        r
    }

    /// Inverso por Fermat (`a^(m-2)`): so vale para `m` PRIMO -- o primo da
    /// curva e a ordem dela, nunca um modulo RSA.
    pub fn inverso_primo(&self, a: &[u64]) -> Vec<u64> {
        let mut dois = vec![0u64; self.m.len()];
        dois[0] = 2;
        let (e, _) = subtrair_n(&self.m, &dois);
        let tam = self.m.len() * 8;
        let e_be = para_be(&e, tam).expect("m - 2 cabe no tamanho de m");
        self.potencia(a, &e_be)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Conferencia contra a aritmetica do `u128`, que e de outra mao: modulos
    /// pequenos de uma palavra, todos os pares de uma faixa.
    #[test]
    fn mul_e_potencia_conferem_com_u128() {
        for m in [
            3u64,
            101,
            65_537,
            0xffff_ffff_ffff_ffc5,
            0x8000_0000_0000_0001,
        ] {
            let md = Modulo::novo(&m.to_be_bytes()).unwrap();
            for a in [0u64, 1, 2, 77, m - 1, m / 2] {
                for b in [0u64, 1, 5, m - 1, m / 3] {
                    let esperado = (a as u128 * b as u128 % m as u128) as u64;
                    let r = md.sair(&md.mul(&md.entrar(&[a]), &md.entrar(&[b])));
                    assert_eq!(r, vec![esperado], "{a}*{b} mod {m}");
                }
                // a^65537 = (a^(2^16)) * a, por quadrados no u128.
                let mut x = a as u128 % m as u128;
                for _ in 0..16 {
                    x = x * x % m as u128;
                }
                let esperado = (x * (a as u128 % m as u128) % m as u128) as u64;
                let r = md.sair(&md.potencia(&md.entrar(&[a]), &[1, 0, 1]));
                assert_eq!(r, vec![esperado], "{a}^65537 mod {m}");
            }
        }
    }

    /// Modulo de varias palavras contra a identidade de Fermat num primo
    /// conhecido (o `p` da P-384, lido do `openssl ecparam -text`):
    /// `a * a^-1 = 1` e `a^(p-1) = 1`.
    #[test]
    fn inverso_e_fermat_no_primo_da_p384() {
        let p = crate::hash::de_hex(
            "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff0000000000000000ffffffff",
        )
        .unwrap();
        let md = Modulo::novo(&p).unwrap();
        for a in [2u64, 3, 0xdead_beef, u64::MAX] {
            let am = md.entrar(&[a, 7, 0, 9]);
            let inv = md.inverso_primo(&am);
            assert_eq!(md.sair(&md.mul(&am, &inv)), md.ajustar(&[1]).unwrap());
        }
    }

    #[test]
    fn modulo_par_ou_pequeno_se_recusa() {
        assert!(Modulo::novo(&[0x10, 0x00]).is_none());
        assert!(Modulo::novo(&[1]).is_none());
        assert!(Modulo::novo(&[0, 0, 5]).is_some());
    }
}
