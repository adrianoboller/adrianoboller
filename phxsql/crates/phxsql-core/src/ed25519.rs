//! Ed25519: assinatura com chave publica e privada, conferida contra a
//! RFC 8032.
//!
//! # Para que serve aqui
//!
//! A senha prova que voce *sabe* alguma coisa. A chave prova que voce *tem*
//! alguma coisa. Sao fatores diferentes, e por isso somam: quem copiar o
//! `config.json` fica com a chave PUBLICA, que nao assina nada.
//!
//! E a diferenca que importa em relacao ao desafio-resposta: la, o que esta
//! guardado no servidor e exatamente a chave usada na prova -- quem le o
//! arquivo consegue autenticar. Aqui nao. A chave privada nunca esteve no
//! servidor, e nao ha nada no `config.json` que permita forjar uma assinatura.
//!
//! # Zero dependencia, e o que isso obriga
//!
//! Nada disto pode "parecer certo". A RFC 8032 traz vetores de teste com
//! chave, mensagem e assinatura; os quatro estao aqui embaixo, mais o vetor
//! de 1023 bytes e o SHA(abc). Se um dia alguem mexer na aritmetica e os
//! vetores continuarem passando, a mexida esta certa. Se pararem, esta errada
//! -- nao importa quao razoavel pareca.
//!
//! # O que esta implementacao NAO promete
//!
//! Nao e tempo constante na multiplicacao escalar. Para o servidor isso nao
//! importa: ele so VERIFICA, e verificar so mexe com dado publico -- nao ha
//! segredo cujo tempo possa vazar. Assinar acontece na maquina de quem tem a
//! chave, e a geracao acontece uma vez, na linha de comando. Quem precisar
//! assinar dentro de um servico exposto deve saber disso.

use crate::sha512::sha512;

// ---------------------------------------------------------------- o corpo
//
// Aritmetica modulo p = 2^255 - 19, com o numero partido em cinco pedacos de
// 51 bits. Cinco de 51 e nao quatro de 64 porque sobram 13 bits de folga em
// cada pedaco: da para somar e multiplicar varias vezes antes de precisar
// propagar o carrego, e o carrego e onde mora o erro.

// `pub(crate)` daqui para baixo por causa do `x25519.rs`: as duas curvas moram
// no MESMO corpo finito, e escrever um segundo `fe_mul` ao lado deste dobraria
// a superficie de erro justamente na parte que ninguem revisa duas vezes.
pub(crate) type Fe = [u64; 5];

const MASCARA: u64 = (1 << 51) - 1;
/// 2p, pedaco a pedaco. Entra na subtracao para nunca haver emprestimo.
const DOIS_P_0: u64 = 0xFFFFFFFFFFFDA;
const DOIS_P_N: u64 = 0xFFFFFFFFFFFFE;

const ZERO: Fe = [0, 0, 0, 0, 0];
const UM: Fe = [1, 0, 0, 0, 0];

/// d = -121665/121666, a constante da curva.
const D: Fe = [
    929955233495203,
    466365720129213,
    1662059464998953,
    2033849074728123,
    1442794654840575,
];
/// 2d, que aparece na formula de soma em coordenadas estendidas.
const D2: Fe = [
    1859910466990425,
    932731440258426,
    1072319116312658,
    1815898335770999,
    633789495995903,
];
/// Uma raiz quadrada de -1. Usada para descomprimir um ponto.
const RAIZ_DE_MENOS_UM: Fe = [
    1718705420411056,
    234908883556509,
    2233514472574048,
    2117202627021982,
    765476049583133,
];

fn fe_carregar(mut h: Fe) -> Fe {
    let mut c;
    for i in 0..4 {
        c = h[i] >> 51;
        h[i] &= MASCARA;
        h[i + 1] += c;
    }
    c = h[4] >> 51;
    h[4] &= MASCARA;
    // O que passa do topo volta multiplicado por 19: e o que 2^255 = 19 diz.
    h[0] += 19 * c;
    c = h[0] >> 51;
    h[0] &= MASCARA;
    h[1] += c;
    h
}

pub(crate) fn fe_soma(a: Fe, b: Fe) -> Fe {
    fe_carregar([
        a[0] + b[0],
        a[1] + b[1],
        a[2] + b[2],
        a[3] + b[3],
        a[4] + b[4],
    ])
}

pub(crate) fn fe_sub(a: Fe, b: Fe) -> Fe {
    fe_carregar([
        a[0] + DOIS_P_0 - b[0],
        a[1] + DOIS_P_N - b[1],
        a[2] + DOIS_P_N - b[2],
        a[3] + DOIS_P_N - b[3],
        a[4] + DOIS_P_N - b[4],
    ])
}

fn fe_neg(a: Fe) -> Fe {
    fe_sub(ZERO, a)
}

pub(crate) fn fe_mul(a: Fe, b: Fe) -> Fe {
    let (a0, a1, a2, a3, a4) = (
        a[0] as u128,
        a[1] as u128,
        a[2] as u128,
        a[3] as u128,
        a[4] as u128,
    );
    let (b0, b1, b2, b3, b4) = (
        b[0] as u128,
        b[1] as u128,
        b[2] as u128,
        b[3] as u128,
        b[4] as u128,
    );
    // Os termos que passariam de 2^255 voltam multiplicados por 19.
    let r0 = a0 * b0 + 19 * (a1 * b4 + a2 * b3 + a3 * b2 + a4 * b1);
    let r1 = a0 * b1 + a1 * b0 + 19 * (a2 * b4 + a3 * b3 + a4 * b2);
    let r2 = a0 * b2 + a1 * b1 + a2 * b0 + 19 * (a3 * b4 + a4 * b3);
    let r3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + 19 * (a4 * b4);
    let r4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

    let mut h = [0u64; 5];
    let mut c: u128 = 0;
    for (i, r) in [r0, r1, r2, r3, r4].into_iter().enumerate() {
        let v = r + c;
        h[i] = (v & MASCARA as u128) as u64;
        c = v >> 51;
    }
    h[0] += 19 * c as u64;
    fe_carregar(h)
}

pub(crate) fn fe_quadrado(a: Fe) -> Fe {
    fe_mul(a, a)
}

/// `a^(2^n) * b`, o tijolo das cadeias de exponenciacao.
fn fe_quadrados(mut a: Fe, n: u32) -> Fe {
    for _ in 0..n {
        a = fe_quadrado(a);
    }
    a
}

/// a^(p-2) = a^-1. Cadeia de adicao classica do curve25519.
pub(crate) fn fe_inverso(z: Fe) -> Fe {
    let z2 = fe_quadrado(z);
    let z8 = fe_quadrados(z2, 2);
    let z9 = fe_mul(z, z8);
    let z11 = fe_mul(z2, z9);
    let z22 = fe_quadrado(z11);
    let z_5_0 = fe_mul(z9, z22);
    let z_10_5 = fe_quadrados(z_5_0, 5);
    let z_10_0 = fe_mul(z_10_5, z_5_0);
    let z_20_10 = fe_quadrados(z_10_0, 10);
    let z_20_0 = fe_mul(z_20_10, z_10_0);
    let z_40_20 = fe_quadrados(z_20_0, 20);
    let z_40_0 = fe_mul(z_40_20, z_20_0);
    let z_50_10 = fe_quadrados(z_40_0, 10);
    let z_50_0 = fe_mul(z_50_10, z_10_0);
    let z_100_50 = fe_quadrados(z_50_0, 50);
    let z_100_0 = fe_mul(z_100_50, z_50_0);
    let z_200_100 = fe_quadrados(z_100_0, 100);
    let z_200_0 = fe_mul(z_200_100, z_100_0);
    let z_250_50 = fe_quadrados(z_200_0, 50);
    let z_250_0 = fe_mul(z_250_50, z_50_0);
    let z_255_5 = fe_quadrados(z_250_0, 5);
    fe_mul(z_255_5, z11)
}

/// a^((p-5)/8). Entra na raiz quadrada da descompressao.
fn fe_pow_p58(z: Fe) -> Fe {
    let z2 = fe_quadrado(z);
    let z8 = fe_quadrados(z2, 2);
    let z9 = fe_mul(z, z8);
    let z11 = fe_mul(z2, z9);
    let z22 = fe_quadrado(z11);
    let z_5_0 = fe_mul(z9, z22);
    let z_10_5 = fe_quadrados(z_5_0, 5);
    let z_10_0 = fe_mul(z_10_5, z_5_0);
    let z_20_10 = fe_quadrados(z_10_0, 10);
    let z_20_0 = fe_mul(z_20_10, z_10_0);
    let z_40_20 = fe_quadrados(z_20_0, 20);
    let z_40_0 = fe_mul(z_40_20, z_20_0);
    let z_50_10 = fe_quadrados(z_40_0, 10);
    let z_50_0 = fe_mul(z_50_10, z_10_0);
    let z_100_50 = fe_quadrados(z_50_0, 50);
    let z_100_0 = fe_mul(z_100_50, z_50_0);
    let z_200_100 = fe_quadrados(z_100_0, 100);
    let z_200_0 = fe_mul(z_200_100, z_100_0);
    let z_250_50 = fe_quadrados(z_200_0, 50);
    let z_250_0 = fe_mul(z_250_50, z_50_0);
    let z_252_2 = fe_quadrados(z_250_0, 2);
    fe_mul(z_252_2, z)
}

pub(crate) fn fe_de_bytes(b: &[u8; 32]) -> Fe {
    let carregar = |i: usize, n: usize| -> u64 {
        let mut v = 0u64;
        for k in 0..n {
            v |= (b[i + k] as u64) << (8 * k);
        }
        v
    };
    // Oito bytes em cada leitura, sempre. Com sete, o pedaco do meio perde o
    // bit 152 -- e o defeito passa despercebido, porque o ponto base tem esse
    // bit em zero.
    [
        carregar(0, 8) & MASCARA,
        (carregar(6, 8) >> 3) & MASCARA,
        (carregar(12, 8) >> 6) & MASCARA,
        (carregar(19, 8) >> 1) & MASCARA,
        // O bit 255 e o sinal, e nao faz parte do numero.
        (carregar(24, 8) >> 12) & MASCARA,
    ]
}

pub(crate) fn fe_para_bytes(h: Fe) -> [u8; 32] {
    // Reduz de vez: soma 19, olha se passou de 2^255, e desconta p se passou.
    let mut h = fe_carregar(h);
    let mut q = (h[0] + 19) >> 51;
    q = (h[1] + q) >> 51;
    q = (h[2] + q) >> 51;
    q = (h[3] + q) >> 51;
    q = (h[4] + q) >> 51;
    h[0] += 19 * q;
    let mut c = h[0] >> 51;
    h[0] &= MASCARA;
    for pedaco in h.iter_mut().skip(1) {
        *pedaco += c;
        c = *pedaco >> 51;
        *pedaco &= MASCARA;
    }

    let mut s = [0u8; 32];
    let mut acumulado: u128 = 0;
    let mut bits = 0u32;
    let mut i = 0usize;
    for pedaco in h {
        acumulado |= (pedaco as u128) << bits;
        bits += 51;
        while bits >= 8 && i < 32 {
            s[i] = (acumulado & 0xff) as u8;
            acumulado >>= 8;
            bits -= 8;
            i += 1;
        }
    }
    while i < 32 {
        s[i] = (acumulado & 0xff) as u8;
        acumulado >>= 8;
        i += 1;
    }
    s
}

fn fe_e_zero(a: Fe) -> bool {
    fe_para_bytes(a) == [0u8; 32]
}

fn fe_negativo(a: Fe) -> bool {
    fe_para_bytes(a)[0] & 1 == 1
}

fn fe_iguais(a: Fe, b: Fe) -> bool {
    fe_para_bytes(a) == fe_para_bytes(b)
}

// ---------------------------------------------------------------- o ponto
//
// Coordenadas estendidas (X:Y:Z:T), com x = X/Z, y = Y/Z e x*y = T/Z. O T
// existe so para a soma nao precisar de inversao -- inverter e caro, e a
// soma acontece 255 vezes por multiplicacao escalar.

#[derive(Clone, Copy, Debug)]
struct Ponto {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

const NEUTRO: Ponto = Ponto {
    x: ZERO,
    y: UM,
    z: UM,
    t: ZERO,
};

fn ponto_dobro(p: &Ponto) -> Ponto {
    let a = fe_quadrado(p.x);
    let b = fe_quadrado(p.y);
    let c = fe_soma(fe_quadrado(p.z), fe_quadrado(p.z));
    let h = fe_soma(a, b);
    let e = fe_sub(h, fe_quadrado(fe_soma(p.x, p.y)));
    let g = fe_sub(a, b);
    let f = fe_soma(c, g);
    Ponto {
        x: fe_mul(e, f),
        y: fe_mul(g, h),
        t: fe_mul(e, h),
        z: fe_mul(f, g),
    }
}

fn ponto_soma(p: &Ponto, q: &Ponto) -> Ponto {
    let a = fe_mul(fe_sub(p.y, p.x), fe_sub(q.y, q.x));
    let b = fe_mul(fe_soma(p.y, p.x), fe_soma(q.y, q.x));
    let c = fe_mul(fe_mul(p.t, q.t), D2);
    let d = fe_soma(fe_mul(p.z, q.z), fe_mul(p.z, q.z));
    let e = fe_sub(b, a);
    let f = fe_sub(d, c);
    let g = fe_soma(d, c);
    let h = fe_soma(b, a);
    Ponto {
        x: fe_mul(e, f),
        y: fe_mul(g, h),
        t: fe_mul(e, h),
        z: fe_mul(f, g),
    }
}

/// `escalar * p`, com o escalar em 32 bytes little-endian.
fn ponto_mul(escalar: &[u8; 32], p: &Ponto) -> Ponto {
    let mut r = NEUTRO;
    // Do bit mais alto para o mais baixo: dobra sempre, soma quando o bit e 1.
    for i in (0..256).rev() {
        r = ponto_dobro(&r);
        if (escalar[i / 8] >> (i % 8)) & 1 == 1 {
            r = ponto_soma(&r, p);
        }
    }
    r
}

fn ponto_comprimir(p: &Ponto) -> [u8; 32] {
    let inv = fe_inverso(p.z);
    let x = fe_mul(p.x, inv);
    let y = fe_mul(p.y, inv);
    let mut s = fe_para_bytes(y);
    // O y cabe em 255 bits; o bit que sobra guarda o sinal do x.
    s[31] |= u8::from(fe_negativo(x)) << 7;
    s
}

/// Recupera o ponto a partir dos 32 bytes. `None` se nao houver ponto nenhum
/// com esse y -- que e o caso de quase toda cadeia de bytes ao acaso.
fn ponto_descomprimir(s: &[u8; 32]) -> Option<Ponto> {
    let y = fe_de_bytes(s);
    let y2 = fe_quadrado(y);
    // x^2 = (y^2 - 1) / (d*y^2 + 1)
    let u = fe_sub(y2, UM);
    let v = fe_soma(fe_mul(D, y2), UM);

    // Raiz quadrada por Tonelli-Shanks especializado para p = 5 mod 8.
    let v3 = fe_mul(fe_quadrado(v), v);
    let v7 = fe_mul(fe_quadrado(v3), v);
    let mut x = fe_mul(fe_mul(u, v3), fe_pow_p58(fe_mul(u, v7)));

    let conferir = fe_mul(v, fe_quadrado(x));
    if !fe_iguais(conferir, u) {
        if fe_iguais(conferir, fe_neg(u)) {
            // A outra raiz: multiplica por sqrt(-1).
            x = fe_mul(x, RAIZ_DE_MENOS_UM);
        } else {
            return None; // nao ha ponto com esse y
        }
    }

    if fe_e_zero(x) && (s[31] >> 7) == 1 {
        return None; // x = 0 com sinal negativo nao existe
    }
    if fe_negativo(x) != ((s[31] >> 7) == 1) {
        x = fe_neg(x);
    }
    Some(Ponto {
        t: fe_mul(x, y),
        x,
        y,
        z: UM,
    })
}

/// O ponto base, na forma comprimida da RFC 8032.
///
/// Descomprimido em vez de escrito como constante: menos numero magico, e a
/// propria descompressao fica exercitada em todo uso.
fn base() -> Ponto {
    let mut b = [0x66u8; 32];
    b[0] = 0x58;
    ponto_descomprimir(&b).expect("o ponto base da RFC 8032 esta na curva")
}

// -------------------------------------------------------------- o escalar
//
// Aritmetica modulo L, a ordem do grupo:
//   L = 2^252 + 27742317777372353535851937790883648493

const L: [u64; 4] = [
    0x5812631a5cf5d3ed,
    0x14def9dea2f79cd6,
    0x0000000000000000,
    0x1000000000000000,
];

/// `a >= L`?
fn maior_ou_igual_l(a: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        match a[i].cmp(&L[i]) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    true
}

fn sub_l(a: &mut [u64; 4]) {
    let mut emprestimo = 0u64;
    for i in 0..4 {
        let (v, e1) = a[i].overflowing_sub(L[i]);
        let (v, e2) = v.overflowing_sub(emprestimo);
        a[i] = v;
        emprestimo = u64::from(e1 || e2);
    }
}

/// `2a mod L`.
fn dobrar_mod_l(a: &mut [u64; 4]) {
    let mut carrego = 0u64;
    for parte in a.iter_mut() {
        let novo = (*parte << 1) | carrego;
        carrego = *parte >> 63;
        *parte = novo;
    }
    // L < 2^253, entao um numero < L dobrado cabe em 254 bits: no maximo
    // duas subtracoes bastam, e o carrego so aparece se ja passou.
    if carrego == 1 || maior_ou_igual_l(a) {
        sub_l(a);
    }
    if maior_ou_igual_l(a) {
        sub_l(a);
    }
}

fn somar_byte_mod_l(a: &mut [u64; 4], b: u8) {
    let mut carrego = b as u64;
    for parte in a.iter_mut() {
        let (v, estourou) = parte.overflowing_add(carrego);
        *parte = v;
        carrego = u64::from(estourou);
        if carrego == 0 {
            break;
        }
    }
    if maior_ou_igual_l(a) {
        sub_l(a);
    }
}

/// Reduz um numero de qualquer tamanho modulo L, byte a byte.
///
/// Horner sobre base 256: `acc = acc*256 + byte`, e `*256` sao oito
/// duplicacoes. Ha jeito mais rapido (Barrett, Montgomery); este e o jeito
/// que da para conferir lendo, e o custo -- alguns milhares de operacoes de
/// 64 bits por assinatura -- nao aparece em lugar nenhum.
fn reduzir_mod_l(bytes_le: &[u8]) -> [u8; 32] {
    let mut acc = [0u64; 4];
    for b in bytes_le.iter().rev() {
        for _ in 0..8 {
            dobrar_mod_l(&mut acc);
        }
        somar_byte_mod_l(&mut acc, *b);
    }
    let mut saida = [0u8; 32];
    for (i, parte) in acc.iter().enumerate() {
        saida[i * 8..i * 8 + 8].copy_from_slice(&parte.to_le_bytes());
    }
    saida
}

/// `(a*b + c) mod L`, tudo em 32 bytes little-endian.
fn mul_soma_mod_l(a: &[u8; 32], b: &[u8; 32], c: &[u8; 32]) -> [u8; 32] {
    // Produto de 256x256 em base 2^32, para o parcial caber em u64.
    let peda = |x: &[u8; 32]| -> [u64; 8] {
        let mut p = [0u64; 8];
        for (i, item) in p.iter_mut().enumerate() {
            *item = u32::from_le_bytes(x[i * 4..i * 4 + 4].try_into().unwrap()) as u64;
        }
        p
    };
    let (pa, pb) = (peda(a), peda(b));
    let mut prod = [0u64; 17];
    for i in 0..8 {
        let mut carrego = 0u64;
        for j in 0..8 {
            let v = prod[i + j] + pa[i] * pb[j] + carrego;
            prod[i + j] = v & 0xffff_ffff;
            carrego = v >> 32;
        }
        prod[i + 8] += carrego;
    }
    let mut bytes = Vec::with_capacity(68);
    for parte in prod {
        bytes.extend_from_slice(&(parte as u32).to_le_bytes());
        debug_assert!(parte <= u32::MAX as u64 || bytes.len() > 64);
    }
    bytes.truncate(68);

    let reduzido = reduzir_mod_l(&bytes);
    // Soma o c e reduz de novo.
    let mut acc = [0u64; 4];
    for i in 0..4 {
        acc[i] = u64::from_le_bytes(reduzido[i * 8..i * 8 + 8].try_into().unwrap());
    }
    let mut soma = Vec::with_capacity(33);
    let mut carrego = 0u64;
    for i in 0..4 {
        let cc = u64::from_le_bytes(c[i * 8..i * 8 + 8].try_into().unwrap());
        let (v, e1) = acc[i].overflowing_add(cc);
        let (v, e2) = v.overflowing_add(carrego);
        carrego = u64::from(e1 || e2);
        soma.extend_from_slice(&v.to_le_bytes());
    }
    soma.push(carrego as u8);
    reduzir_mod_l(&soma)
}

// ------------------------------------------------------------ a interface

pub const CHAVE_LEN: usize = 32;
pub const ASSINATURA_LEN: usize = 64;

/// A chave publica que corresponde a esta chave privada.
pub fn chave_publica(privada: &[u8; CHAVE_LEN]) -> [u8; CHAVE_LEN] {
    let h = sha512(privada);
    let a = escalar_da_semente(&h);
    ponto_comprimir(&ponto_mul(&a, &base()))
}

/// O escalar sai dos 32 primeiros bytes do hash, com os bits ajustados como
/// manda a RFC: os tres de baixo zerados (para o resultado cair no subgrupo)
/// e o de cima fixado (para toda multiplicacao ter o mesmo tamanho).
fn escalar_da_semente(h: &[u8; 64]) -> [u8; 32] {
    let mut a = [0u8; 32];
    a.copy_from_slice(&h[..32]);
    a[0] &= 248;
    a[31] &= 127;
    a[31] |= 64;
    a
}

/// Assina a mensagem.
pub fn assinar(privada: &[u8; CHAVE_LEN], mensagem: &[u8]) -> [u8; ASSINATURA_LEN] {
    let h = sha512(privada);
    let a = escalar_da_semente(&h);
    let publica = ponto_comprimir(&ponto_mul(&a, &base()));

    // r = H(prefixo || M). O prefixo e a segunda metade do hash da chave: e o
    // que faz duas assinaturas da mesma mensagem sairem iguais sem precisar
    // de sorteio. Assinatura deterministica nao depende do gerador de
    // aleatorios da maquina -- que e onde muita implementacao se perdeu.
    let mut ctx = crate::sha512::Sha512::novo();
    ctx.atualizar(&h[32..]);
    ctx.atualizar(mensagem);
    let r = reduzir_mod_l(&ctx.finalizar());

    let ponto_r = ponto_comprimir(&ponto_mul(&r, &base()));

    let mut ctx = crate::sha512::Sha512::novo();
    ctx.atualizar(&ponto_r);
    ctx.atualizar(&publica);
    ctx.atualizar(mensagem);
    let k = reduzir_mod_l(&ctx.finalizar());

    let s = mul_soma_mod_l(&k, &a, &r);

    let mut assinatura = [0u8; ASSINATURA_LEN];
    assinatura[..32].copy_from_slice(&ponto_r);
    assinatura[32..].copy_from_slice(&s);
    assinatura
}

/// Confere a assinatura. Falso para qualquer coisa que nao bata -- e nunca
/// entra em panico, porque a entrada vem da rede.
pub fn conferir(
    publica: &[u8; CHAVE_LEN],
    mensagem: &[u8],
    assinatura: &[u8; ASSINATURA_LEN],
) -> bool {
    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&assinatura[..32]);
    let mut s = [0u8; 32];
    s.copy_from_slice(&assinatura[32..]);

    // S tem de estar reduzido. Sem esta conferencia a assinatura seria
    // maleavel: daria para produzir uma segunda assinatura valida da mesma
    // mensagem somando L ao S.
    let mut s64 = [0u64; 4];
    for i in 0..4 {
        s64[i] = u64::from_le_bytes(s[i * 8..i * 8 + 8].try_into().unwrap());
    }
    if maior_ou_igual_l(&s64) {
        return false;
    }

    let Some(ponto_a) = ponto_descomprimir(publica) else {
        return false;
    };
    let Some(ponto_r) = ponto_descomprimir(&r_bytes) else {
        return false;
    };

    let mut ctx = crate::sha512::Sha512::novo();
    ctx.atualizar(&r_bytes);
    ctx.atualizar(publica);
    ctx.atualizar(mensagem);
    let k = reduzir_mod_l(&ctx.finalizar());

    // Confere [S]B == R + [k]A.
    let esquerda = ponto_mul(&s, &base());
    let direita = ponto_soma(&ponto_r, &ponto_mul(&k, &ponto_a));
    ponto_comprimir(&esquerda) == ponto_comprimir(&direita)
}

/// Uma chave privada nova, do gerador do sistema.
pub fn gerar_privada() -> [u8; CHAVE_LEN] {
    let mut k = [0u8; CHAVE_LEN];
    k.copy_from_slice(&crate::senha::bytes_aleatorios(CHAVE_LEN));
    k
}

/// 32 bytes a partir de 64 hexadecimais.
pub fn chave_de_hex(s: &str) -> Option<[u8; CHAVE_LEN]> {
    let b = crate::hash::de_hex(s.trim())?;
    if b.len() != CHAVE_LEN {
        return None;
    }
    let mut k = [0u8; CHAVE_LEN];
    k.copy_from_slice(&b);
    Some(k)
}

/// 64 bytes a partir de 128 hexadecimais.
pub fn assinatura_de_hex(s: &str) -> Option<[u8; ASSINATURA_LEN]> {
    let b = crate::hash::de_hex(s.trim())?;
    if b.len() != ASSINATURA_LEN {
        return None;
    }
    let mut a = [0u8; ASSINATURA_LEN];
    a.copy_from_slice(&b);
    Some(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{de_hex, para_hex};

    fn k32(h: &str) -> [u8; 32] {
        chave_de_hex(h).unwrap()
    }

    /// RFC 8032, secao 7.1. Estes quatro vetores sao o criterio: se passarem,
    /// as chaves conversam com qualquer outra implementacao de Ed25519.
    #[test]
    fn vetores_da_rfc_8032() {
        struct Vetor {
            privada: &'static str,
            publica: &'static str,
            mensagem: &'static str,
            assinatura: &'static str,
        }
        let vetores = [
            // TEST 1: mensagem vazia
            Vetor {
                privada: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
                publica: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                mensagem: "",
                assinatura: "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155\
                             5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
            },
            // TEST 2: um byte
            Vetor {
                privada: "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
                publica: "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
                mensagem: "72",
                assinatura: "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da\
                             085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
            },
            // TEST 3: dois bytes
            Vetor {
                privada: "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
                publica: "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
                mensagem: "af82",
                assinatura: "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac\
                             18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
            },
            // TEST SHA(abc): a mensagem e o SHA-512 de "abc"
            Vetor {
                privada: "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
                publica: "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
                mensagem: "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                           2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
                assinatura: "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b589\
                             09351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
            },
        ];

        for (i, v) in vetores.iter().enumerate() {
            let privada = k32(v.privada);
            let mensagem = de_hex(&v.mensagem.replace([' ', '\n'], "")).unwrap();

            assert_eq!(
                para_hex(&chave_publica(&privada)),
                v.publica,
                "vetor {}: a chave publica saiu errada",
                i + 1
            );

            let assinada = assinar(&privada, &mensagem);
            assert_eq!(
                para_hex(&assinada),
                v.assinatura.replace([' ', '\n'], ""),
                "vetor {}: a assinatura saiu errada",
                i + 1
            );

            let publica = k32(v.publica);
            let esperada = assinatura_de_hex(&v.assinatura.replace([' ', '\n'], "")).unwrap();
            assert!(
                conferir(&publica, &mensagem, &esperada),
                "vetor {}: nao conferiu a propria assinatura",
                i + 1
            );
        }
    }

    #[test]
    fn o_vetor_de_1023_bytes() {
        // TEST 1024 da RFC: a mensagem longa exercita o SHA-512 em varios
        // blocos dentro da assinatura.
        let privada = k32("f5e5767cf153319517630f226876b86c8160cc583bc013744c6bf255f5cc0ee5");
        let publica = k32("278117fc144c72340f67d0f2316e8386ceffbf2b2428c9c51fef7c597f1d426e");
        let mensagem = de_hex(
            "08b8b2b733424243760fe426a4b54908632110a66c2f6591eabd3345e3e4eb98\
             fa6e264bf09efe12ee50f8f54e9f77b1e355f6c50544e23fb1433ddf73be84d8\
             79de7c0046dc4996d9e773f4bc9efe5738829adb26c81b37c93a1b270b20329d\
             658675fc6ea534e0810a4432826bf58c941efb65d57a338bbd2e26640f89ffbc\
             1a858efcb8550ee3a5e1998bd177e93a7363c344fe6b199ee5d02e82d522c4fe\
             ba15452f80288a821a579116ec6dad2b3b310da903401aa62100ab5d1a36553e\
             06203b33890cc9b832f79ef80560ccb9a39ce767967ed628c6ad573cb116dbef\
             efd75499da96bd68a8a97b928a8bbc103b6621fcde2beca1231d206be6cd9ec7\
             aff6f6c94fcd7204ed3455c68c83f4a41da4af2b74ef5c53f1d8ac70bdcb7ed1\
             85ce81bd84359d44254d95629e9855a94a7c1958d1f8ada5d0532ed8a5aa3fb2\
             d17ba70eb6248e594e1a2297acbbb39d502f1a8c6eb6f1ce22b3de1a1f40cc24\
             554119a831a9aad6079cad88425de6bde1a9187ebb6092cf67bf2b13fd65f270\
             88d78b7e883c8759d2c4f5c65adb7553878ad575f9fad878e80a0c9ba63bcbcc\
             2732e69485bbc9c90bfbd62481d9089beccf80cfe2df16a2cf65bd92dd597b07\
             07e0917af48bbb75fed413d238f5555a7a569d80c3414a8d0859dc65a46128ba\
             b27af87a71314f318c782b23ebfe808b82b0ce26401d2e22f04d83d1255dc51a\
             ddd3b75a2b1ae0784504df543af8969be3ea7082ff7fc9888c144da2af58429e\
             c96031dbcad3dad9af0dcbaaaf268cb8fcffead94f3c7ca495e056a9b47acdb7\
             51fb73e666c6c655ade8297297d07ad1ba5e43f1bca32301651339e22904cc8c\
             42f58c30c04aafdb038dda0847dd988dcda6f3bfd15c4b4c4525004aa06eeff8\
             ca61783aacec57fb3d1f92b0fe2fd1a85f6724517b65e614ad6808d6f6ee34df\
             f7310fdc82aebfd904b01e1dc54b2927094b2db68d6f903b68401adebf5a7e08\
             d78ff4ef5d63653a65040cf9bfd4aca7984a74d37145986780fc0b16ac451649\
             de6188a7dbdf191f64b5fc5e2ab47b57f7f7276cd419c17a3ca8e1b939ae49e4\
             88acba6b965610b5480109c8b17b80e1b7b750dfc7598d5d5011fd2dcc5600a3\
             2ef5b52a1ecc820e308aa342721aac0943bf6686b64b2579376504ccc493d97e\
             6aed3fb0f9cd71a43dd497f01f17c0e2cb3797aa2a2f256656168e6c496afc5f\
             b93246f6b1116398a346f1a641f3b041e989f7914f90cc2c7fff357876e506b5\
             0d334ba77c225bc307ba537152f3f1610e4eafe595f6d9d90d11faa933a15ef1\
             369546868a7f3a45a96768d40fd9d03412c091c6315cf4fde7cb68606937380d\
             b2eaaa707b4c4185c32eddcdd306705e4dc1ffc872eeee475a64dfac86aba41c\
             0618983f8741c5ef68d3a101e8a3b8cac60c905c15fc910840b94c00a0b9d0",
        )
        .unwrap();
        let assinatura = assinatura_de_hex(
            "0aab4c900501b3e24d7cdf4663326a3a87df5e4843b2cbdb67cbf6e460fec350\
             aa5371b1508f9f4528ecea23c436d94b5e8fcd4f681e30a6ac00a9704a188a03",
        )
        .unwrap();

        assert_eq!(chave_publica(&privada), publica);
        assert_eq!(assinar(&privada, &mensagem), assinatura);
        assert!(conferir(&publica, &mensagem, &assinatura));
    }

    #[test]
    fn assinatura_torta_nao_passa() {
        let privada = k32("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let publica = chave_publica(&privada);
        let msg = b"transferir 10 reais";
        let boa = assinar(&privada, msg);
        assert!(conferir(&publica, msg, &boa));

        // Mensagem trocada.
        assert!(!conferir(&publica, b"transferir 99 reais", &boa));

        // Um bit virado em cada metade da assinatura.
        for i in [0usize, 31, 32, 63] {
            let mut torta = boa;
            torta[i] ^= 1;
            assert!(
                !conferir(&publica, msg, &torta),
                "passou com o byte {i} virado"
            );
        }

        // Outra chave publica.
        let outra = chave_publica(&k32(
            "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        ));
        assert!(!conferir(&outra, msg, &boa));
    }

    #[test]
    fn s_fora_da_faixa_e_recusado() {
        // Maleabilidade: somar L ao S daria outra assinatura da MESMA
        // mensagem, valida pela equacao da curva. Tem de ser recusada.
        let privada = k32("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let publica = chave_publica(&privada);
        let msg = b"sempre a mesma mensagem";
        let boa = assinar(&privada, msg);

        let mut maleavel = boa;
        let mut s = [0u64; 4];
        for i in 0..4 {
            s[i] = u64::from_le_bytes(boa[32 + i * 8..32 + i * 8 + 8].try_into().unwrap());
        }
        let mut carrego = 0u64;
        for i in 0..4 {
            let (v, e1) = s[i].overflowing_add(L[i]);
            let (v, e2) = v.overflowing_add(carrego);
            carrego = u64::from(e1 || e2);
            maleavel[32 + i * 8..32 + i * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        assert!(conferir(&publica, msg, &boa));
        assert!(
            !conferir(&publica, msg, &maleavel),
            "S >= L tem de ser recusado, senao a assinatura e maleavel"
        );
    }

    #[test]
    fn bytes_que_nao_sao_ponto_nao_derrubam() {
        // A entrada vem da rede: nada aqui pode entrar em panico.
        let privada = k32("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let boa = assinar(&privada, b"x");
        for enchimento in [0x00u8, 0x01, 0xff, 0x7f] {
            let lixo = [enchimento; 32];
            assert!(!conferir(&lixo, b"x", &boa) || enchimento == 0x00);
            let mut assinatura_lixo = [enchimento; 64];
            assinatura_lixo[63] = 0; // S pequeno, R e que sera lixo
            assert!(!conferir(&chave_publica(&privada), b"x", &assinatura_lixo));
        }
    }

    #[test]
    fn a_chave_gerada_assina_e_confere() {
        let privada = gerar_privada();
        let publica = chave_publica(&privada);
        let msg = b"nonce-do-servidor,nonce-do-cliente,adriano";
        assert!(conferir(&publica, msg, &assinar(&privada, msg)));
        // Duas chaves geradas nao saem iguais.
        assert_ne!(privada, gerar_privada());
    }

    #[test]
    fn assinar_e_deterministico() {
        // Sem sorteio: a mesma chave e a mesma mensagem dao sempre a mesma
        // assinatura. E o que tira o gerador de aleatorios do caminho critico.
        let privada = k32("c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7");
        assert_eq!(assinar(&privada, b"igual"), assinar(&privada, b"igual"));
    }

    #[test]
    fn o_corpo_se_comporta() {
        // Aritmetica basica, para um erro no carrego aparecer aqui em vez de
        // sair como "a assinatura nao confere" tres camadas acima.
        let dois = fe_soma(UM, UM);
        assert!(fe_iguais(fe_mul(dois, dois), fe_soma(dois, dois)));
        assert!(fe_e_zero(fe_sub(dois, dois)));
        assert!(fe_iguais(fe_mul(dois, fe_inverso(dois)), UM));
        assert!(fe_iguais(fe_soma(dois, fe_neg(dois)), ZERO));
        // p-1 vai e volta pelos bytes sem se perder.
        let mut quase_p = [0xffu8; 32];
        quase_p[0] = 0xec;
        quase_p[31] = 0x7f;
        assert_eq!(fe_para_bytes(fe_de_bytes(&quase_p)), quase_p);
    }

    #[test]
    fn o_ponto_base_tem_a_ordem_certa() {
        // L * B = neutro. E a definicao da ordem do grupo, e uma conta que so
        // fecha se a curva, o escalar e a multiplicacao estiverem os tres
        // certos ao mesmo tempo.
        let mut l = [0u8; 32];
        for (i, parte) in L.iter().enumerate() {
            l[i * 8..i * 8 + 8].copy_from_slice(&parte.to_le_bytes());
        }
        let r = ponto_mul(&l, &base());
        assert_eq!(ponto_comprimir(&r), ponto_comprimir(&NEUTRO));
    }

    #[test]
    fn comprimir_e_descomprimir_vai_e_volta() {
        let b = base();
        let bytes = ponto_comprimir(&b);
        let volta = ponto_descomprimir(&bytes).unwrap();
        assert_eq!(ponto_comprimir(&volta), bytes);
        // Dobro e soma consigo mesmo tem de dar o mesmo ponto.
        assert_eq!(
            ponto_comprimir(&ponto_dobro(&b)),
            ponto_comprimir(&ponto_soma(&b, &b))
        );
    }

    #[test]
    fn hexadecimal_recusa_tamanho_errado() {
        assert!(chave_de_hex("").is_none());
        assert!(chave_de_hex("ab").is_none());
        assert!(chave_de_hex(&"a".repeat(63)).is_none());
        assert!(chave_de_hex(&"a".repeat(64)).is_some());
        assert!(chave_de_hex(&"a".repeat(66)).is_none());
        assert!(chave_de_hex("zz".repeat(32).as_str()).is_none());
        assert!(assinatura_de_hex(&"a".repeat(128)).is_some());
        assert!(assinatura_de_hex(&"a".repeat(126)).is_none());
    }
}
