//! QR Code (ISO/IEC 18004), modos byte e alfanumerico, para o lado de quem
//! paga renderizar o BR Code na hora. Sem dependencias externas -- so a `std`.
//!
//! # O que se prova, e contra o que
//!
//! O risco desta peca e a matematica de correcao de erro: um Reed-Solomon
//! errado corrige para o lado errado e ninguem ve. Por isso a prova principal
//! e contra **vetor externo publicado**, e nao "parece certo":
//!
//! * **Reed-Solomon e codificacao de dados** -- o exemplo trabalhado de
//!   `"HELLO WORLD"` em modo alfanumerico, versao 1, nivel M, cujos codewords
//!   de dados E de ECC saem publicados byte a byte no tutorial de QR
//!   (thonky.com) e no anexo de exemplos da ISO/IEC 18004. Os dez codewords de
//!   ECC batendo e a prova de que o GF(256) e o gerador estao certos.
//! * **Format info (BCH 15,5)** -- as 32 strings de formato publicadas
//!   (4 niveis x 8 mascaras) sao reproduzidas pelo `formato_bits`.
//! * **Version info (BCH 18,6)** -- as strings das versoes 7 a 10.
//! * **Tabela de blocos e de alinhamento** -- transcritas da mesma fonte.
//!
//! O resto -- posicionamento na matriz, mascara e o leitor -- se prova por
//! **ida e volta**: o leitor minimo daqui decodifica a propria saida e devolve
//! a entrada; e com defeito reposto (modulos virados) ele corrige dentro da
//! capacidade e FALHA, sem devolver lixo, acima dela.
//!
//! A convencao de posicionamento (format/version/ziguezague) segue a do
//! gerador de referencia do Nayuki, conferida contra os vetores acima.

use crate::error::{PhxError, Result};

// ------------------------------------------------------------------------
// GF(256), o corpo do Reed-Solomon do QR: polinomio primitivo 0x11D, alpha=2.
// ------------------------------------------------------------------------

/// Tabelas exp/log. `exp` tem 512 posicoes (dobrada) para o produto dispensar
/// o resto por 255: `log[a] + log[b]` nunca passa de 508.
const fn gf_tabelas() -> ([u8; 512], [u8; 256]) {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut x: u16 = 1;
    let mut i = 0usize;
    while i < 255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x & 0x100 != 0 {
            x ^= 0x11D;
        }
        i += 1;
    }
    let mut j = 255usize;
    while j < 512 {
        exp[j] = exp[j - 255];
        j += 1;
    }
    (exp, log)
}

static GF: ([u8; 512], [u8; 256]) = gf_tabelas();

fn exp(i: usize) -> u8 {
    GF.0[i]
}
fn log(x: u8) -> usize {
    GF.1[x as usize] as usize
}
/// Produto no GF(256). Zero e absorvente; fora disso, soma de logaritmos.
fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        GF.0[log(a) + log(b)]
    }
}
/// Inverso multiplicativo (o argumento tem de ser diferente de zero).
fn inv(a: u8) -> u8 {
    GF.0[255 - log(a)]
}

// ------------------------------------------------------------------------
// Reed-Solomon: geracao dos codewords de ECC.
// ------------------------------------------------------------------------

/// Multiplica dois polinomios sobre GF(256) (mesma ordem em ambos).
fn poly_mul(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut r = vec![0u8; a.len() + b.len() - 1];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            r[i + j] ^= mul(x, y);
        }
    }
    r
}

/// Polinomio gerador de grau `n`: produto de `(x - alpha^i)`, i em 0..n.
/// Ordenacao "maior grau primeiro", com coeficiente lider 1.
fn gerador(n: usize) -> Vec<u8> {
    let mut g = vec![1u8];
    for i in 0..n {
        g = poly_mul(&g, &[1, exp(i)]);
    }
    g
}

/// Os `n` codewords de ECC de um bloco: o resto da divisao de `dados * x^n`
/// pelo gerador. E divisao sintetica; como o coeficiente lider do gerador e 1,
/// cada passo zera um coeficiente dos dados.
fn ecc_bloco(dados: &[u8], n: usize) -> Vec<u8> {
    let gen = gerador(n);
    let mut buf = vec![0u8; dados.len() + n];
    buf[..dados.len()].copy_from_slice(dados);
    for i in 0..dados.len() {
        let coef = buf[i];
        if coef != 0 {
            for (j, &g) in gen.iter().enumerate() {
                buf[i + j] ^= mul(g, coef);
            }
        }
    }
    buf[dados.len()..].to_vec()
}

// ------------------------------------------------------------------------
// Niveis de correcao e a tabela de blocos (versoes 1..=10).
// ------------------------------------------------------------------------

/// Nivel de correcao de erro. Ordem de robustez crescente L < M < Q < H.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nivel {
    L,
    M,
    Q,
    H,
}

fn nivel_idx(n: Nivel) -> usize {
    match n {
        Nivel::L => 0,
        Nivel::M => 1,
        Nivel::Q => 2,
        Nivel::H => 3,
    }
}

/// Os 2 bits do nivel no format info (indicador do padrao: M=00, L=01, H=10, Q=11).
fn nivel_bits(n: Nivel) -> u16 {
    match n {
        Nivel::L => 0b01,
        Nivel::M => 0b00,
        Nivel::Q => 0b11,
        Nivel::H => 0b10,
    }
}

/// Parametros de bloco de uma (versao, nivel).
#[derive(Clone, Copy)]
struct Param {
    /// Codewords de ECC por bloco.
    ec: u8,
    g1_bloc: u8,
    g1_dad: u8,
    g2_bloc: u8,
    g2_dad: u8,
}

impl Param {
    fn total_dados(&self) -> usize {
        self.g1_bloc as usize * self.g1_dad as usize + self.g2_bloc as usize * self.g2_dad as usize
    }
    fn num_blocos(&self) -> usize {
        self.g1_bloc as usize + self.g2_bloc as usize
    }
    fn total_codewords(&self) -> usize {
        self.total_dados() + self.num_blocos() * self.ec as usize
    }
}

const fn pr(ec: u8, g1b: u8, g1d: u8, g2b: u8, g2d: u8) -> Param {
    Param {
        ec,
        g1_bloc: g1b,
        g1_dad: g1d,
        g2_bloc: g2b,
        g2_dad: g2d,
    }
}

/// Tabela de blocos, versoes 1..=10, na ordem de nivel [L, M, Q, H].
///
/// Transcrita da tabela de correcao de erro publicada (ISO/IEC 18004 /
/// tutorial de QR). Numeros errados aqui erram calado, entao o vetor de
/// `"HELLO WORLD"` (V1-M) prende a primeira linha e a soma por versao confere
/// o resto: `total_codewords` tem de bater com os codewords totais da versao.
const PARAMS: [[Param; 4]; 10] = [
    // V1
    [
        pr(7, 1, 19, 0, 0),
        pr(10, 1, 16, 0, 0),
        pr(13, 1, 13, 0, 0),
        pr(17, 1, 9, 0, 0),
    ],
    // V2
    [
        pr(10, 1, 34, 0, 0),
        pr(16, 1, 28, 0, 0),
        pr(22, 1, 22, 0, 0),
        pr(28, 1, 16, 0, 0),
    ],
    // V3
    [
        pr(15, 1, 55, 0, 0),
        pr(26, 1, 44, 0, 0),
        pr(18, 2, 17, 0, 0),
        pr(22, 2, 13, 0, 0),
    ],
    // V4
    [
        pr(20, 1, 80, 0, 0),
        pr(18, 2, 32, 0, 0),
        pr(26, 2, 24, 0, 0),
        pr(16, 4, 9, 0, 0),
    ],
    // V5
    [
        pr(26, 1, 108, 0, 0),
        pr(24, 2, 43, 0, 0),
        pr(18, 2, 15, 2, 16),
        pr(22, 2, 11, 2, 12),
    ],
    // V6
    [
        pr(18, 2, 68, 0, 0),
        pr(16, 4, 27, 0, 0),
        pr(24, 4, 19, 0, 0),
        pr(28, 4, 15, 0, 0),
    ],
    // V7
    [
        pr(20, 2, 78, 0, 0),
        pr(18, 4, 31, 0, 0),
        pr(18, 2, 14, 4, 15),
        pr(26, 4, 13, 1, 14),
    ],
    // V8
    [
        pr(24, 2, 97, 0, 0),
        pr(22, 2, 38, 2, 39),
        pr(22, 4, 18, 2, 19),
        pr(26, 4, 14, 2, 15),
    ],
    // V9
    [
        pr(30, 2, 116, 0, 0),
        pr(22, 3, 36, 2, 37),
        pr(20, 4, 16, 4, 17),
        pr(24, 4, 12, 4, 13),
    ],
    // V10
    [
        pr(18, 2, 68, 2, 69),
        pr(26, 4, 43, 1, 44),
        pr(24, 6, 19, 2, 20),
        pr(28, 6, 15, 2, 16),
    ],
];

/// Codewords totais por versao (dados + ECC), para conferir a tabela de blocos.
/// So o teste a usa -- e o "gabarito" contra o qual a tabela digitada e somada.
#[cfg(test)]
const CODEWORDS_POR_VERSAO: [usize; 10] = [26, 44, 70, 100, 134, 172, 196, 242, 292, 346];

fn param(versao: u8, nivel: Nivel) -> Param {
    PARAMS[versao as usize - 1][nivel_idx(nivel)]
}

/// Centros dos padroes de alinhamento por versao. O produto cartesiano desta
/// lista da os centros, menos os que caem sobre os localizadores.
fn alinhamento_centros(versao: u8) -> &'static [usize] {
    match versao {
        2 => &[6, 18],
        3 => &[6, 22],
        4 => &[6, 26],
        5 => &[6, 30],
        6 => &[6, 34],
        7 => &[6, 22, 38],
        8 => &[6, 24, 42],
        9 => &[6, 26, 46],
        10 => &[6, 28, 50],
        _ => &[],
    }
}

/// Lado da matriz, em modulos: `17 + 4*versao`.
fn dimensao(versao: u8) -> usize {
    17 + 4 * versao as usize
}

// ------------------------------------------------------------------------
// Codificacao dos dados no fluxo de bits.
// ------------------------------------------------------------------------

/// Modo de codificacao. Byte serve para qualquer octeto (o BR Code);
/// alfanumerico e mais denso, mas so cobre `0-9 A-Z` e alguns simbolos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modo {
    Byte,
    Alfanumerico,
}

/// Bits empilhados MSB primeiro.
struct Bits(Vec<bool>);

impl Bits {
    fn novo() -> Self {
        Bits(Vec::new())
    }
    fn push(&mut self, valor: u32, n: u32) {
        for i in (0..n).rev() {
            self.0.push((valor >> i) & 1 == 1);
        }
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Valor de um caractere no modo alfanumerico, ou `None` se ele nao existe la.
fn alnum_valor(c: u8) -> Option<u32> {
    Some(match c {
        b'0'..=b'9' => (c - b'0') as u32,
        b'A'..=b'Z' => (c - b'A') as u32 + 10,
        b' ' => 36,
        b'$' => 37,
        b'%' => 38,
        b'*' => 39,
        b'+' => 40,
        b'-' => 41,
        b'.' => 42,
        b'/' => 43,
        b':' => 44,
        _ => return None,
    })
}

/// Caractere a partir do valor alfanumerico (o inverso, para o leitor).
fn alnum_char(v: u8) -> u8 {
    match v {
        0..=9 => b'0' + v,
        10..=35 => b'A' + (v - 10),
        36 => b' ',
        37 => b'$',
        38 => b'%',
        39 => b'*',
        40 => b'+',
        41 => b'-',
        42 => b'.',
        43 => b'/',
        _ => b':',
    }
}

/// Numero de bits do indicador de contagem de caracteres, por modo e versao.
fn bits_contagem(modo: Modo, versao: u8) -> u32 {
    match modo {
        Modo::Byte => {
            if versao <= 9 {
                8
            } else {
                16
            }
        }
        Modo::Alfanumerico => {
            if versao <= 9 {
                9
            } else if versao <= 26 {
                11
            } else {
                13
            }
        }
    }
}

fn bits_para_bytes(bits: &[bool]) -> Vec<u8> {
    let mut out = vec![0u8; bits.len() / 8];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            out[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    out
}

/// Codifica `dados` no fluxo de codewords de DADOS (antes da intercalacao),
/// ja com terminador e preenchimento (0xEC / 0x11) ate a capacidade.
fn codificar_dados(dados: &[u8], modo: Modo, versao: u8, nivel: Nivel) -> Result<Vec<u8>> {
    let cap = param(versao, nivel).total_dados();
    let cap_bits = cap * 8;

    let mut b = Bits::novo();
    let indicador = match modo {
        Modo::Byte => 0b0100,
        Modo::Alfanumerico => 0b0010,
    };
    b.push(indicador, 4);
    b.push(dados.len() as u32, bits_contagem(modo, versao));

    match modo {
        Modo::Byte => {
            for &x in dados {
                b.push(x as u32, 8);
            }
        }
        Modo::Alfanumerico => {
            let nao_alnum =
                |x: u8| PhxError::Esquema(format!("byte {x} nao existe no modo alfanumerico"));
            let mut i = 0;
            while i + 1 < dados.len() {
                let a = alnum_valor(dados[i]).ok_or_else(|| nao_alnum(dados[i]))?;
                let c = alnum_valor(dados[i + 1]).ok_or_else(|| nao_alnum(dados[i + 1]))?;
                b.push(a * 45 + c, 11);
                i += 2;
            }
            if i < dados.len() {
                let a = alnum_valor(dados[i]).ok_or_else(|| nao_alnum(dados[i]))?;
                b.push(a, 6);
            }
        }
    }

    if b.len() > cap_bits {
        return Err(PhxError::LimiteExcedido(format!(
            "dados nao cabem na versao {versao} nivel {nivel:?}: {} bits > {cap_bits} bits",
            b.len()
        )));
    }
    // Terminador: ate 4 zeros, sem passar da capacidade.
    let term = 4.min(cap_bits - b.len());
    b.push(0, term as u32);
    // Completa o ultimo byte.
    while b.0.len() % 8 != 0 {
        b.0.push(false);
    }
    let mut bytes = bits_para_bytes(&b.0);
    // Preenchimento com o par 0xEC / 0x11 ate encher a capacidade.
    let mut ec = true;
    while bytes.len() < cap {
        bytes.push(if ec { 0xEC } else { 0x11 });
        ec = !ec;
    }
    Ok(bytes)
}

/// Intercala os blocos de dados e de ECC como o QR exige na matriz.
fn intercalar(dados: &[u8], versao: u8, nivel: Nivel) -> Vec<u8> {
    let p = param(versao, nivel);
    let mut blocos: Vec<Vec<u8>> = Vec::new();
    let mut pos = 0;
    for _ in 0..p.g1_bloc {
        blocos.push(dados[pos..pos + p.g1_dad as usize].to_vec());
        pos += p.g1_dad as usize;
    }
    for _ in 0..p.g2_bloc {
        blocos.push(dados[pos..pos + p.g2_dad as usize].to_vec());
        pos += p.g2_dad as usize;
    }
    let ec = p.ec as usize;
    let blocos_ec: Vec<Vec<u8>> = blocos.iter().map(|d| ecc_bloco(d, ec)).collect();

    let max_dad = blocos.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::with_capacity(p.total_codewords());
    for col in 0..max_dad {
        for bloco in &blocos {
            if col < bloco.len() {
                out.push(bloco[col]);
            }
        }
    }
    for col in 0..ec {
        for bloco in &blocos_ec {
            out.push(bloco[col]);
        }
    }
    out
}

// ------------------------------------------------------------------------
// A matriz de modulos e os padroes fixos.
// ------------------------------------------------------------------------

#[derive(Clone)]
struct Matriz {
    dim: usize,
    /// `true` = modulo escuro.
    mods: Vec<bool>,
    /// `true` = modulo de funcao (localizador, tempo, formato...) -- nao carrega dado.
    func: Vec<bool>,
}

impl Matriz {
    fn nova(dim: usize) -> Self {
        Matriz {
            dim,
            mods: vec![false; dim * dim],
            func: vec![false; dim * dim],
        }
    }
    fn get(&self, r: usize, c: usize) -> bool {
        self.mods[r * self.dim + c]
    }
    fn set(&mut self, r: usize, c: usize, v: bool) {
        self.mods[r * self.dim + c] = v;
    }
    /// Grava um modulo de funcao: cor e marca de "reservado" ao mesmo tempo.
    fn set_f(&mut self, r: usize, c: usize, v: bool) {
        let i = r * self.dim + c;
        self.mods[i] = v;
        self.func[i] = true;
    }
    fn reservar(&mut self, r: usize, c: usize) {
        self.func[r * self.dim + c] = true;
    }
    fn eh_func(&self, r: usize, c: usize) -> bool {
        self.func[r * self.dim + c]
    }
}

/// Desenha um localizador 7x7 com o separador de 1 modulo em volta, dado o
/// canto superior-esquerdo do 7x7.
fn finder(m: &mut Matriz, top: isize, left: isize) {
    let dim = m.dim as isize;
    for dr in -1..=7isize {
        for dc in -1..=7isize {
            let r = top + dr;
            let c = left + dc;
            if r < 0 || c < 0 || r >= dim || c >= dim {
                continue;
            }
            let dentro = (0..=6).contains(&dr) && (0..=6).contains(&dc);
            let escuro = dentro
                && (dr == 0
                    || dr == 6
                    || dc == 0
                    || dc == 6
                    || ((2..=4).contains(&dr) && (2..=4).contains(&dc)));
            m.set_f(r as usize, c as usize, escuro);
        }
    }
}

/// Desenha um padrao de alinhamento 5x5 centrado em `(cr, cc)`.
fn alinhamento(m: &mut Matriz, cr: usize, cc: usize) {
    for dr in -2..=2isize {
        for dc in -2..=2isize {
            let r = (cr as isize + dr) as usize;
            let c = (cc as isize + dc) as usize;
            // Centro escuro, anel do meio claro (distancia 1), anel externo escuro.
            let escuro = dr.abs().max(dc.abs()) != 1;
            m.set_f(r, c, escuro);
        }
    }
}

/// Um centro de alinhamento cai sobre um dos tres localizadores?
fn sobrepoe_finder(r: usize, c: usize, dim: usize) -> bool {
    let perto = |x: usize| x <= 8;
    let longe = |x: usize| x >= dim - 9;
    (perto(r) && perto(c)) || (perto(r) && longe(c)) || (longe(r) && perto(c))
}

/// Posicoes dos 15 bits do format info, copia 1 (em volta do localizador
/// superior-esquerdo), na ordem do bit 0 ao 14.
fn pos_formato1() -> [(usize, usize); 15] {
    [
        (0, 8),
        (1, 8),
        (2, 8),
        (3, 8),
        (4, 8),
        (5, 8),
        (7, 8),
        (8, 8),
        (8, 7),
        (8, 5),
        (8, 4),
        (8, 3),
        (8, 2),
        (8, 1),
        (8, 0),
    ]
}

/// Posicoes da copia 2 do format info (borda direita da linha 8 e base da
/// coluna 8), na ordem do bit 0 ao 14.
fn pos_formato2(dim: usize) -> [(usize, usize); 15] {
    [
        (8, dim - 1),
        (8, dim - 2),
        (8, dim - 3),
        (8, dim - 4),
        (8, dim - 5),
        (8, dim - 6),
        (8, dim - 7),
        (8, dim - 8),
        (dim - 7, 8),
        (dim - 6, 8),
        (dim - 5, 8),
        (dim - 4, 8),
        (dim - 3, 8),
        (dim - 2, 8),
        (dim - 1, 8),
    ]
}

/// As 18 posicoes de cada uma das duas copias do version info (v >= 7).
fn pos_versao(dim: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::with_capacity(36);
    for i in 0..18usize {
        let a = dim - 11 + i % 3;
        let b = i / 3;
        v.push((b, a)); // bloco superior-direito
        v.push((a, b)); // bloco inferior-esquerdo
    }
    v
}

/// Desenha os padroes fixos (localizadores, tempo, alinhamento, modulo escuro)
/// e RESERVA (sem cor final) as celulas de formato e de versao.
fn desenhar_fixos(versao: u8) -> Matriz {
    let dim = dimensao(versao);
    let mut m = Matriz::nova(dim);

    finder(&mut m, 0, 0);
    finder(&mut m, 0, dim as isize - 7);
    finder(&mut m, dim as isize - 7, 0);

    // Padroes de tempo nas linha/coluna 6, so onde ainda nao ha funcao.
    for i in 0..dim {
        if !m.eh_func(6, i) {
            m.set_f(6, i, i % 2 == 0);
        }
        if !m.eh_func(i, 6) {
            m.set_f(i, 6, i % 2 == 0);
        }
    }

    // Alinhamento: produto dos centros, menos os que batem nos localizadores.
    let centros = alinhamento_centros(versao);
    for &r in centros {
        for &c in centros {
            if !sobrepoe_finder(r, c, dim) {
                alinhamento(&mut m, r, c);
            }
        }
    }

    // Modulo sempre escuro.
    m.set_f(dim - 8, 8, true);

    // Reserva as celulas de format info (cor final entra por mascara escolhida).
    for (r, c) in pos_formato1() {
        m.reservar(r, c);
    }
    for (r, c) in pos_formato2(dim) {
        m.reservar(r, c);
    }
    if versao >= 7 {
        for (r, c) in pos_versao(dim) {
            m.reservar(r, c);
        }
    }
    m
}

// ------------------------------------------------------------------------
// Format info e version info (codigos BCH).
// ------------------------------------------------------------------------

/// Os 15 bits do format info para (nivel, mascara). BCH(15,5) com gerador
/// 0x537, mascarado por 0x5412.
fn formato_bits(nivel: Nivel, mascara: u8) -> u16 {
    let dados = (nivel_bits(nivel) << 3) | mascara as u16; // 5 bits
    let mut rem = dados;
    for _ in 0..10 {
        rem = (rem << 1) ^ (((rem >> 9) & 1) * 0x0537);
    }
    ((dados << 10) | (rem & 0x3FF)) ^ 0x5412
}

/// Os 18 bits do version info (v >= 7). BCH(18,6) com gerador 0x1F25.
fn versao_bits(versao: u8) -> u32 {
    let mut rem: u32 = versao as u32;
    for _ in 0..12 {
        rem = (rem << 1) ^ (((rem >> 11) & 1) * 0x1F25);
    }
    ((versao as u32) << 12) | (rem & 0xFFF)
}

/// Grava as duas copias do format info na matriz, mais o modulo escuro.
fn aplicar_formato(m: &mut Matriz, nivel: Nivel, mascara: u8) {
    let bits = formato_bits(nivel, mascara);
    let p1 = pos_formato1();
    let p2 = pos_formato2(m.dim);
    for i in 0..15 {
        let bit = (bits >> i) & 1 == 1;
        m.set_f(p1[i].0, p1[i].1, bit);
        m.set_f(p2[i].0, p2[i].1, bit);
    }
    m.set_f(m.dim - 8, 8, true);
}

/// Grava as duas copias do version info (v >= 7).
fn aplicar_versao(m: &mut Matriz, versao: u8) {
    let bits = versao_bits(versao);
    let dim = m.dim;
    for i in 0..18usize {
        let bit = (bits >> i) & 1 == 1;
        let a = dim - 11 + i % 3;
        let b = i / 3;
        m.set_f(b, a, bit);
        m.set_f(a, b, bit);
    }
}

// ------------------------------------------------------------------------
// Colocacao dos dados em ziguezague, mascara e penalidade.
// ------------------------------------------------------------------------

/// Coloca os bits dos codewords na matriz, em ziguezague de baixo para cima,
/// duas colunas por vez, pulando a coluna 6 (tempo) e as celulas de funcao.
fn colocar_dados(m: &mut Matriz, cw: &[u8]) {
    let dim = m.dim as isize;
    let total_bits = cw.len() * 8;
    let mut i = 0usize;
    let mut right = dim - 1;
    while right >= 1 {
        if right == 6 {
            right = 5;
        }
        for vert in 0..dim {
            for j in 0..2 {
                let col = (right - j) as usize;
                let sobe = ((right + 1) & 2) == 0;
                let row = if sobe { dim - 1 - vert } else { vert } as usize;
                if !m.eh_func(row, col) && i < total_bits {
                    let bit = (cw[i >> 3] >> (7 - (i & 7))) & 1 == 1;
                    m.set(row, col, bit);
                    i += 1;
                }
            }
        }
        right -= 2;
    }
}

fn mascara_condicao(mascara: u8, r: usize, c: usize) -> bool {
    let (r, c) = (r as u64, c as u64);
    match mascara {
        0 => (r + c) % 2 == 0,
        1 => r % 2 == 0,
        2 => c % 3 == 0,
        3 => (r + c) % 3 == 0,
        4 => (r / 2 + c / 3) % 2 == 0,
        5 => (r * c) % 2 + (r * c) % 3 == 0,
        6 => ((r * c) % 2 + (r * c) % 3) % 2 == 0,
        _ => ((r + c) % 2 + (r * c) % 3) % 2 == 0,
    }
}

fn aplicar_mascara(m: &mut Matriz, mascara: u8) {
    for r in 0..m.dim {
        for c in 0..m.dim {
            if !m.eh_func(r, c) && mascara_condicao(mascara, r, c) {
                let v = m.get(r, c);
                m.set(r, c, !v);
            }
        }
    }
}

/// Regra 1 da penalidade sobre uma linha ou coluna: 5 iguais seguidos valem 3,
/// e cada modulo a mais na sequencia soma 1.
fn penal_sequencia<F: Fn(usize) -> bool>(f: F, dim: usize) -> u32 {
    let mut total = 0u32;
    let mut run = 1u32;
    for i in 1..dim {
        if f(i) == f(i - 1) {
            run += 1;
        } else {
            if run >= 5 {
                total += 3 + (run - 5);
            }
            run = 1;
        }
    }
    if run >= 5 {
        total += 3 + (run - 5);
    }
    total
}

/// Padrao tipo-localizador (regra 3): escuro-claro-escuro-escuro-escuro-claro-
/// escuro com quatro claros de um lado, nos dois sentidos.
const FINDER_A: [bool; 11] = [
    true, false, true, true, true, false, true, false, false, false, false,
];
const FINDER_B: [bool; 11] = [
    false, false, false, false, true, false, true, true, true, false, true,
];

fn penal_finder<F: Fn(usize) -> bool>(f: F, dim: usize) -> u32 {
    if dim < 11 {
        return 0;
    }
    let mut total = 0u32;
    for i in 0..=dim - 11 {
        let mut a = true;
        let mut b = true;
        for k in 0..11 {
            let v = f(i + k);
            a &= v == FINDER_A[k];
            b &= v == FINDER_B[k];
        }
        if a {
            total += 40;
        }
        if b {
            total += 40;
        }
    }
    total
}

/// Penalidade total de uma matriz mascarada (soma das quatro regras).
fn penalidade(m: &Matriz) -> u32 {
    let dim = m.dim;
    let mut total = 0u32;

    // Regra 1.
    for r in 0..dim {
        total += penal_sequencia(|c| m.get(r, c), dim);
    }
    for c in 0..dim {
        total += penal_sequencia(|r| m.get(r, c), dim);
    }
    // Regra 2: blocos 2x2 da mesma cor (contando sobreposicoes).
    for r in 0..dim - 1 {
        for c in 0..dim - 1 {
            let a = m.get(r, c);
            if a == m.get(r, c + 1) && a == m.get(r + 1, c) && a == m.get(r + 1, c + 1) {
                total += 3;
            }
        }
    }
    // Regra 3.
    for r in 0..dim {
        total += penal_finder(|c| m.get(r, c), dim);
    }
    for c in 0..dim {
        total += penal_finder(|r| m.get(r, c), dim);
    }
    // Regra 4: desvio da metade escura, em passos de 5%.
    let escuros = m.mods.iter().filter(|&&b| b).count() as i64;
    let n = (dim * dim) as i64;
    let k = ((escuros * 20 - n * 10).abs() + n - 1) / n - 1;
    total += (k.max(0) as u32) * 10;

    total
}

// ------------------------------------------------------------------------
// API publica: gerar e ler.
// ------------------------------------------------------------------------

/// Um QR Code pronto: a versao, o nivel, a mascara escolhida e a matriz.
pub struct Qr {
    pub versao: u8,
    pub nivel: Nivel,
    pub mascara: u8,
    dim: usize,
    modulos: Vec<bool>,
}

impl Qr {
    /// Lado da matriz, em modulos.
    pub fn dim(&self) -> usize {
        self.dim
    }
    /// O modulo em `(linha, coluna)` e escuro?
    pub fn escuro(&self, r: usize, c: usize) -> bool {
        self.modulos[r * self.dim + c]
    }
}

/// Gera um QR em modo byte, na menor versao (ate 10) que couber no nivel dado.
pub fn gerar(dados: &[u8], nivel: Nivel) -> Result<Qr> {
    for versao in 1..=10u8 {
        if cabe(dados.len(), Modo::Byte, versao, nivel) {
            return gerar_versao(dados, versao, nivel, Modo::Byte);
        }
    }
    Err(PhxError::LimiteExcedido(format!(
        "{} bytes nao cabem em QR versao <=10 no nivel {nivel:?}",
        dados.len()
    )))
}

fn cabe(qtd: usize, modo: Modo, versao: u8, nivel: Nivel) -> bool {
    let cap_bits = param(versao, nivel).total_dados() * 8;
    let conteudo = 4 + bits_contagem(modo, versao) as usize + qtd * 8;
    conteudo <= cap_bits
}

/// Gera um QR numa versao e modo fixos, escolhendo a melhor mascara por
/// menor penalidade.
pub fn gerar_versao(dados: &[u8], versao: u8, nivel: Nivel, modo: Modo) -> Result<Qr> {
    if !(1..=10).contains(&versao) {
        return Err(PhxError::Esquema(format!(
            "versao {versao} fora de 1..=10 nesta build"
        )));
    }
    let cw_dados = codificar_dados(dados, modo, versao, nivel)?;
    let cw = intercalar(&cw_dados, versao, nivel);

    let mut com_dados = desenhar_fixos(versao);
    colocar_dados(&mut com_dados, &cw);

    let mut melhor_pen = u32::MAX;
    let mut melhor_msk = 0u8;
    let mut melhor: Option<Matriz> = None;
    for mascara in 0..8u8 {
        let mut m = com_dados.clone();
        aplicar_mascara(&mut m, mascara);
        aplicar_formato(&mut m, nivel, mascara);
        if versao >= 7 {
            aplicar_versao(&mut m, versao);
        }
        let pen = penalidade(&m);
        if pen < melhor_pen {
            melhor_pen = pen;
            melhor_msk = mascara;
            melhor = Some(m);
        }
    }
    let m = melhor.expect("sempre ha ao menos uma mascara");
    Ok(Qr {
        versao,
        nivel,
        mascara: melhor_msk,
        dim: dimensao(versao),
        modulos: m.mods,
    })
}

/// Le (decodifica) um QR gerado por esta casa: acha nivel/mascara pelo format
/// info, desmascara, extrai os codewords, corrige por Reed-Solomon e devolve o
/// conteudo. E o leitor minimo que fecha a prova de ida e volta.
pub fn ler(qr: &Qr) -> Result<Vec<u8>> {
    let dim = qr.dim;
    if dim < 21 || (dim - 17) % 4 != 0 {
        return Err(PhxError::Corrompido(format!("QR: dimensao invalida {dim}")));
    }
    let versao = ((dim - 17) / 4) as u8;
    if !(1..=10).contains(&versao) {
        return Err(PhxError::Corrompido(format!(
            "QR: versao {versao} fora do alcance desta build"
        )));
    }

    // Le os 15 bits da copia 1 do format info e descobre (nivel, mascara).
    let p1 = pos_formato1();
    let mut lido = 0u16;
    for (i, &(r, c)) in p1.iter().enumerate() {
        if qr.escuro(r, c) {
            lido |= 1 << i;
        }
    }
    let (nivel, mascara) = decodificar_formato(lido)?;

    // Reconstroi o mapa de funcao e copia a matriz publicada nele.
    let mut m = desenhar_fixos(versao);
    for r in 0..dim {
        for c in 0..dim {
            m.set(r, c, qr.escuro(r, c));
        }
    }
    // Desfaz a mascara nas celulas de dado (XOR e a propria inversa).
    aplicar_mascara(&mut m, mascara);

    let total_cw = param(versao, nivel).total_codewords();
    let cw = ler_codewords(&m, total_cw);
    let dados_cw = desintercalar_e_corrigir(&cw, versao, nivel)?;
    decodificar_bitstream(&dados_cw, versao)
}

/// Descobre (nivel, mascara) pelo format info lido, por menor distancia de
/// Hamming as 32 strings validas -- e assim que um leitor de verdade tolera
/// erro no proprio format info.
fn decodificar_formato(lido: u16) -> Result<(Nivel, u8)> {
    let niveis = [Nivel::L, Nivel::M, Nivel::Q, Nivel::H];
    let mut melhor_d = u32::MAX;
    let mut res = (Nivel::M, 0u8);
    for &nv in &niveis {
        for msk in 0..8u8 {
            let dist = (formato_bits(nv, msk) ^ lido).count_ones();
            if dist < melhor_d {
                melhor_d = dist;
                res = (nv, msk);
            }
        }
    }
    if melhor_d > 3 {
        return Err(PhxError::Corrompido(format!(
            "QR: format info irreconhecivel (distancia {melhor_d})"
        )));
    }
    Ok(res)
}

/// Le `total_cw` codewords da matriz desmascarada, na mesma ordem ziguezague.
fn ler_codewords(m: &Matriz, total_cw: usize) -> Vec<u8> {
    let dim = m.dim as isize;
    let total_bits = total_cw * 8;
    let mut cw = vec![0u8; total_cw];
    let mut i = 0usize;
    let mut right = dim - 1;
    while right >= 1 {
        if right == 6 {
            right = 5;
        }
        for vert in 0..dim {
            for j in 0..2 {
                let col = (right - j) as usize;
                let sobe = ((right + 1) & 2) == 0;
                let row = if sobe { dim - 1 - vert } else { vert } as usize;
                if !m.eh_func(row, col) && i < total_bits {
                    if m.get(row, col) {
                        cw[i >> 3] |= 1 << (7 - (i & 7));
                    }
                    i += 1;
                }
            }
        }
        right -= 2;
    }
    cw
}

/// Desintercala os codewords em blocos e corrige cada um por Reed-Solomon,
/// devolvendo o fluxo de codewords de DADOS ja corrigido.
fn desintercalar_e_corrigir(cw: &[u8], versao: u8, nivel: Nivel) -> Result<Vec<u8>> {
    let p = param(versao, nivel);
    let ec = p.ec as usize;
    let num_blocos = p.num_blocos();

    let mut tam: Vec<usize> = Vec::with_capacity(num_blocos);
    for _ in 0..p.g1_bloc {
        tam.push(p.g1_dad as usize);
    }
    for _ in 0..p.g2_bloc {
        tam.push(p.g2_dad as usize);
    }

    let total_dados = p.total_dados();
    let (dados_inter, ec_inter) = cw.split_at(total_dados);

    let mut blocos: Vec<Vec<u8>> = vec![Vec::new(); num_blocos];
    let max_dad = tam.iter().copied().max().unwrap_or(0);
    let mut idx = 0;
    for col in 0..max_dad {
        for (b, &t) in tam.iter().enumerate() {
            if col < t {
                blocos[b].push(dados_inter[idx]);
                idx += 1;
            }
        }
    }

    let mut blocos_ec: Vec<Vec<u8>> = vec![Vec::new(); num_blocos];
    idx = 0;
    for _ in 0..ec {
        for bloco in blocos_ec.iter_mut() {
            bloco.push(ec_inter[idx]);
            idx += 1;
        }
    }

    let mut saida = Vec::with_capacity(total_dados);
    for b in 0..num_blocos {
        let mut full = blocos[b].clone();
        full.extend_from_slice(&blocos_ec[b]);
        let corrigido = rs_corrigir(&full, ec)?;
        saida.extend_from_slice(&corrigido[..blocos[b].len()]);
    }
    Ok(saida)
}

/// Corrige um codeword completo (dados || ECC) por Reed-Solomon.
///
/// Sindromes -> Berlekamp-Massey -> busca de Chien -> magnitudes por sistema de
/// Vandermonde. Recusa (em vez de devolver lixo) quando o numero de erros passa
/// da capacidade `num_ec/2`, quando as raizes nao batem com o grau, ou quando a
/// correcao nao zera as sindromes.
fn rs_corrigir(full: &[u8], num_ec: usize) -> Result<Vec<u8>> {
    let n = full.len();
    let sindrome = |cw: &[u8], k: usize| -> u8 {
        let mut ev = 0u8;
        for &c in cw {
            ev = mul(ev, exp(k)) ^ c;
        }
        ev
    };

    let mut synd = vec![0u8; num_ec];
    let mut algum = false;
    for (k, s) in synd.iter_mut().enumerate() {
        *s = sindrome(full, k);
        if *s != 0 {
            algum = true;
        }
    }
    if !algum {
        return Ok(full.to_vec());
    }

    let mut loc = berlekamp_massey(&synd);
    while loc.len() > 1 && *loc.last().unwrap() == 0 {
        loc.pop();
    }
    let v = loc.len() - 1;
    if v == 0 || v > num_ec / 2 {
        return Err(PhxError::Corrompido(format!(
            "QR: {v} erros passam da capacidade {}",
            num_ec / 2
        )));
    }

    // Chien: posicoes (potencias de x) onde o localizador zera em alpha^{-pos}.
    let mut posicoes = Vec::new();
    for pos in 0..n {
        let xinv = exp((255 - (pos % 255)) % 255);
        let mut val = 0u8;
        let mut xp = 1u8;
        for &coef in &loc {
            val ^= mul(coef, xp);
            xp = mul(xp, xinv);
        }
        if val == 0 {
            posicoes.push(pos);
        }
    }
    if posicoes.len() != v {
        return Err(PhxError::Corrompido(
            "QR: raizes do localizador nao batem com o grau".into(),
        ));
    }

    let magnit = resolver_magnitudes(&synd[..v], &posicoes)?;

    let mut corr = full.to_vec();
    for (mi, &pos) in posicoes.iter().enumerate() {
        corr[n - 1 - pos] ^= magnit[mi];
    }
    for k in 0..num_ec {
        if sindrome(&corr, k) != 0 {
            return Err(PhxError::Corrompido(
                "QR: correcao nao zerou as sindromes".into(),
            ));
        }
    }
    Ok(corr)
}

/// Algoritmo de Berlekamp-Massey sobre GF(256): das sindromes ao polinomio
/// localizador de erro (ordem "menor grau primeiro", com termo constante 1).
fn berlekamp_massey(synd: &[u8]) -> Vec<u8> {
    let n = synd.len();
    let mut c = vec![1u8];
    let mut b = vec![1u8];
    let mut l = 0usize;
    let mut m = 1usize;
    let mut bb = 1u8;
    for i in 0..n {
        let mut d = synd[i];
        for j in 1..=l {
            if j < c.len() {
                d ^= mul(c[j], synd[i - j]);
            }
        }
        if d == 0 {
            m += 1;
        } else if 2 * l <= i {
            let t = c.clone();
            let coef = mul(d, inv(bb));
            if c.len() < b.len() + m {
                c.resize(b.len() + m, 0);
            }
            for (j, &bj) in b.iter().enumerate() {
                c[j + m] ^= mul(coef, bj);
            }
            l = i + 1 - l;
            b = t;
            bb = d;
            m = 1;
        } else {
            let coef = mul(d, inv(bb));
            if c.len() < b.len() + m {
                c.resize(b.len() + m, 0);
            }
            for (j, &bj) in b.iter().enumerate() {
                c[j + m] ^= mul(coef, bj);
            }
            m += 1;
        }
    }
    c
}

/// Resolve as magnitudes dos erros: sistema `S_k = sum_i Y_i alpha^{k*pos_i}`
/// (Vandermonde) por eliminacao de Gauss-Jordan em GF(256).
fn resolver_magnitudes(synd: &[u8], posicoes: &[usize]) -> Result<Vec<u8>> {
    let v = posicoes.len();
    let mut a = vec![vec![0u8; v + 1]; v];
    for (k, linha) in a.iter_mut().enumerate() {
        for (i, &pos) in posicoes.iter().enumerate() {
            linha[i] = exp((k * pos) % 255);
        }
        linha[v] = synd[k];
    }
    for col in 0..v {
        let mut piv = col;
        while piv < v && a[piv][col] == 0 {
            piv += 1;
        }
        if piv == v {
            return Err(PhxError::Corrompido(
                "QR: sistema singular na correcao".into(),
            ));
        }
        a.swap(col, piv);
        let inv_p = inv(a[col][col]);
        for x in a[col][col..=v].iter_mut() {
            *x = mul(*x, inv_p);
        }
        // Copia da linha-pivo para poder escrever nas outras sem conflito de
        // emprestimo (r != col, entao nao ha aliasing de dado).
        let pivo = a[col].clone();
        for (r, linha) in a.iter_mut().enumerate() {
            if r != col && linha[col] != 0 {
                let f = linha[col];
                for (j, x) in linha.iter_mut().enumerate().skip(col) {
                    *x ^= mul(f, pivo[j]);
                }
            }
        }
    }
    Ok((0..v).map(|i| a[i][v]).collect())
}

/// Le `n` bits (MSB primeiro) do fluxo, avancando `pos`.
fn pega(bits: &[bool], pos: &mut usize, n: usize) -> Result<u32> {
    if *pos + n > bits.len() {
        return Err(PhxError::Corrompido("QR: bitstream curto".into()));
    }
    let mut v = 0u32;
    for _ in 0..n {
        v = (v << 1) | bits[*pos] as u32;
        *pos += 1;
    }
    Ok(v)
}

/// Decodifica o fluxo de codewords de dados de volta ao conteudo original.
fn decodificar_bitstream(dados_cw: &[u8], versao: u8) -> Result<Vec<u8>> {
    let mut bits = Vec::with_capacity(dados_cw.len() * 8);
    for &b in dados_cw {
        for k in (0..8).rev() {
            bits.push((b >> k) & 1 == 1);
        }
    }
    let mut pos = 0;
    let modo = pega(&bits, &mut pos, 4)?;
    match modo {
        0b0100 => {
            let cbits = if versao <= 9 { 8 } else { 16 };
            let n = pega(&bits, &mut pos, cbits)? as usize;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(pega(&bits, &mut pos, 8)? as u8);
            }
            Ok(out)
        }
        0b0010 => {
            let cbits = if versao <= 9 {
                9
            } else if versao <= 26 {
                11
            } else {
                13
            };
            let n = pega(&bits, &mut pos, cbits)? as usize;
            let mut out = Vec::with_capacity(n);
            let mut resto = n;
            while resto >= 2 {
                let val = pega(&bits, &mut pos, 11)?;
                out.push(alnum_char((val / 45) as u8));
                out.push(alnum_char((val % 45) as u8));
                resto -= 2;
            }
            if resto == 1 {
                let val = pega(&bits, &mut pos, 6)?;
                out.push(alnum_char(val as u8));
            }
            Ok(out)
        }
        _ => Err(PhxError::Corrompido(format!(
            "QR: modo {modo} nao suportado pelo leitor"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Vetor externo: "HELLO WORLD", alfanumerico, versao 1, nivel M. --
    // Codewords publicados byte a byte (tutorial de QR / ISO/IEC 18004).
    const HELLO_DADOS: [u8; 16] = [
        32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17,
    ];
    const HELLO_EC: [u8; 10] = [196, 35, 39, 119, 235, 215, 231, 226, 93, 23];

    /// **Vetor externo (a matematica do Reed-Solomon).** Os dez codewords de
    /// ECC do exemplo publicado. Se o GF(256) ou o gerador estiverem errados,
    /// este teste cai -- e e ele que autoriza confiar na correcao das outras
    /// versoes, que usam o MESMO codigo.
    #[test]
    fn reed_solomon_bate_o_vetor_publicado() {
        assert_eq!(ecc_bloco(&HELLO_DADOS, 10), HELLO_EC.to_vec());
    }

    /// **Vetor externo (a codificacao de dados).** Codificar "HELLO WORLD" em
    /// alfanumerico V1-M tem de dar exatamente os 16 codewords de dados
    /// publicados, com o preenchimento 0xEC/0x11 no fim.
    #[test]
    fn codificacao_bate_o_vetor_publicado() {
        let cw = codificar_dados(b"HELLO WORLD", Modo::Alfanumerico, 1, Nivel::M).unwrap();
        assert_eq!(cw, HELLO_DADOS.to_vec());
        // Numa versao de bloco unico, intercalar e so dados seguidos de ECC.
        let inter = intercalar(&cw, 1, Nivel::M);
        let mut esperado = HELLO_DADOS.to_vec();
        esperado.extend_from_slice(&HELLO_EC);
        assert_eq!(inter, esperado);
    }

    /// **Vetor externo (BCH do format info).** As 32 strings publicadas
    /// (4 niveis x 8 mascaras), MSB primeiro.
    #[test]
    fn format_info_bate_as_32_strings() {
        // (nivel, mascara, string de 15 bits)
        let tabela: [(Nivel, u8, &str); 32] = [
            (Nivel::L, 0, "111011111000100"),
            (Nivel::L, 1, "111001011110011"),
            (Nivel::L, 2, "111110110101010"),
            (Nivel::L, 3, "111100010011101"),
            (Nivel::L, 4, "110011000101111"),
            (Nivel::L, 5, "110001100011000"),
            (Nivel::L, 6, "110110001000001"),
            (Nivel::L, 7, "110100101110110"),
            (Nivel::M, 0, "101010000010010"),
            (Nivel::M, 1, "101000100100101"),
            (Nivel::M, 2, "101111001111100"),
            (Nivel::M, 3, "101101101001011"),
            (Nivel::M, 4, "100010111111001"),
            (Nivel::M, 5, "100000011001110"),
            (Nivel::M, 6, "100111110010111"),
            (Nivel::M, 7, "100101010100000"),
            (Nivel::Q, 0, "011010101011111"),
            (Nivel::Q, 1, "011000001101000"),
            (Nivel::Q, 2, "011111100110001"),
            (Nivel::Q, 3, "011101000000110"),
            (Nivel::Q, 4, "010010010110100"),
            (Nivel::Q, 5, "010000110000011"),
            (Nivel::Q, 6, "010111011011010"),
            (Nivel::Q, 7, "010101111101101"),
            (Nivel::H, 0, "001011010001001"),
            (Nivel::H, 1, "001001110111110"),
            (Nivel::H, 2, "001110011100111"),
            (Nivel::H, 3, "001100111010000"),
            (Nivel::H, 4, "000011101100010"),
            (Nivel::H, 5, "000001001010101"),
            (Nivel::H, 6, "000110100001100"),
            (Nivel::H, 7, "000100000111011"),
        ];
        for (nv, msk, s) in tabela {
            let esperado = u16::from_str_radix(s, 2).unwrap();
            assert_eq!(
                formato_bits(nv, msk),
                esperado,
                "falhou {nv:?} mascara {msk}"
            );
        }
    }

    /// **Vetor externo (BCH do version info).** As strings publicadas de v7 a v10.
    #[test]
    fn version_info_bate_as_strings() {
        let tabela: [(u8, &str); 4] = [
            (7, "000111110010010100"),
            (8, "001000010110111100"),
            (9, "001001101010011001"),
            (10, "001010010011010011"),
        ];
        for (v, s) in tabela {
            let esperado = u32::from_str_radix(s, 2).unwrap();
            assert_eq!(versao_bits(v), esperado, "falhou versao {v}");
        }
    }

    /// A tabela de blocos e coerente: os codewords totais de cada versao/nivel
    /// batem com o total publicado da versao. Uma linha digitada errada aqui
    /// (como um `43` no lugar de `27`) quebra esta soma.
    #[test]
    fn tabela_de_blocos_soma_certo() {
        for versao in 1..=10u8 {
            for nivel in [Nivel::L, Nivel::M, Nivel::Q, Nivel::H] {
                let p = param(versao, nivel);
                assert_eq!(
                    p.total_codewords(),
                    CODEWORDS_POR_VERSAO[versao as usize - 1],
                    "versao {versao} nivel {nivel:?}"
                );
            }
        }
    }

    /// GF(256): produto pela soma de logaritmos e coerente com a tabela exp.
    #[test]
    fn gf_produto_coerente() {
        for i in 0..255usize {
            for j in 0..255usize {
                assert_eq!(mul(exp(i), exp(j)), exp((i + j) % 255));
            }
        }
        // Inverso: a * a^-1 = 1 para todo a != 0.
        for a in 1..=255u16 {
            let a = a as u8;
            assert_eq!(mul(a, inv(a)), 1);
        }
    }

    /// **Ida e volta, modo byte.** O leitor decodifica a propria saida e
    /// devolve a entrada -- prova o posicionamento e a mascara de ponta a ponta.
    #[test]
    fn ida_e_volta_byte() {
        for entrada in [
            &b"phxsql"[..],
            &b"00020101021126"[..],
            &[0u8, 255, 1, 254, 128, 7][..],
        ] {
            let qr = gerar(entrada, Nivel::M).unwrap();
            assert_eq!(ler(&qr).unwrap(), entrada);
        }
    }

    /// Ida e volta com o exemplo alfanumerico canonico.
    #[test]
    fn ida_e_volta_alfanumerico() {
        let qr = gerar_versao(b"HELLO WORLD", 1, Nivel::M, Modo::Alfanumerico).unwrap();
        assert_eq!(ler(&qr).unwrap(), b"HELLO WORLD");
    }

    /// Um payload longo cai numa versao >= 7 (com version info e alinhamento),
    /// e ainda assim volta inteiro -- exercita o caminho arriscado da matriz.
    #[test]
    fn ida_e_volta_versao_alta() {
        let entrada: Vec<u8> = (0..120u8).map(|i| b'0' + (i % 10)).collect();
        let qr = gerar(&entrada, Nivel::M).unwrap();
        assert!(qr.versao >= 7, "esperava versao >=7, veio {}", qr.versao);
        assert_eq!(ler(&qr).unwrap(), entrada);
    }

    /// **Prova real nos dois sentidos, no Reed-Solomon.** Dentro da capacidade
    /// (`t = ec/2` erros) o corretor recupera; um erro a mais, arranjado para
    /// nao virar outro codeword valido, tem de FALHAR -- nao devolver lixo.
    #[test]
    fn correcao_recupera_dentro_e_falha_acima() {
        let dados: Vec<u8> = (0..16u8).collect();
        let ec = 10usize; // capacidade t = 5
        let mut full = dados.clone();
        full.extend_from_slice(&ecc_bloco(&dados, ec));

        // Sem erro: devolve igual.
        assert_eq!(rs_corrigir(&full, ec).unwrap(), full);

        // t erros (5): recupera.
        let mut c5 = full.clone();
        for (i, p) in [0usize, 3, 7, 11, 20].iter().enumerate() {
            c5[*p] ^= 0x5A ^ (i as u8 + 1);
        }
        assert_eq!(rs_corrigir(&c5, ec).unwrap(), full);

        // Muitos erros (o bloco inteiro adulterado): tem de recusar, e nao
        // devolver dado corrompido como se fosse certo.
        let mut cx = full.clone();
        for (i, b) in cx.iter_mut().enumerate() {
            *b ^= 0xFF ^ (i as u8);
        }
        assert!(
            rs_corrigir(&cx, ec).is_err(),
            "erro acima da capacidade tinha de falhar"
        );
    }

    /// **Prova real no nivel da matriz.** Virar poucos modulos de dado deixa o
    /// leitor recuperar (a correcao de erro trabalhando de verdade); destruir
    /// um quadrante inteiro tem de falhar em vez de mentir.
    #[test]
    fn leitor_corrige_poucos_modulos_e_falha_em_muitos() {
        let entrada = b"pagamento phxsql 42";
        let qr = gerar(entrada, Nivel::Q).unwrap(); // Q corrige ~25%
        let dim = qr.dim;

        // Vira 3 modulos e confere que ainda le certo.
        let mut mods = qr.modulos.clone();
        for &(r, c) in &[(10usize, 10usize), (10, 11), (11, 10)] {
            mods[r * dim + c] ^= true;
        }
        let ferido = Qr {
            versao: qr.versao,
            nivel: qr.nivel,
            mascara: qr.mascara,
            dim,
            modulos: mods,
        };
        assert_eq!(
            ler(&ferido).unwrap(),
            entrada,
            "3 modulos deviam ser corrigidos"
        );

        // Destroi a metade de baixo inteira (deixando a copia 1 do format info,
        // que fica no alto, intacta): muito acima da capacidade -> Err, nunca
        // um payload errado apresentado como certo.
        let mut mods = qr.modulos.clone();
        for r in dim / 2..dim {
            for c in 0..dim {
                mods[r * dim + c] = (r * 7 + c * 3) % 2 == 0;
            }
        }
        let destruido = Qr {
            versao: qr.versao,
            nivel: qr.nivel,
            mascara: qr.mascara,
            dim,
            modulos: mods,
        };
        assert!(
            ler(&destruido).is_err(),
            "um quadrante destruido tinha de falhar, nao devolver lixo"
        );
    }

    /// Os padroes fixos estao onde deviam: cantos dos localizadores escuros,
    /// modulo escuro presente, e o padrao de tempo alternando.
    #[test]
    fn padroes_fixos_no_lugar() {
        let qr = gerar(b"x", Nivel::M).unwrap();
        // Canto do localizador superior-esquerdo: (0,0) escuro, (0,7) claro (separador).
        assert!(qr.escuro(0, 0));
        assert!(!qr.escuro(7, 7));
        // Modulo sempre escuro.
        assert!(qr.escuro(qr.dim - 8, 8));
        // Padrao de tempo na linha 6: alterna a partir da coluna 8.
        assert!(qr.escuro(6, 8));
        assert!(!qr.escuro(6, 9));
    }
}
