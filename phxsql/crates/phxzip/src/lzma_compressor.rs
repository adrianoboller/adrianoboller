//! LZMA2: o CODIFICADOR -- o que faz o PhxZip comprimir de verdade.
//!
//! # O que ele e, e o que ele nao e
//!
//! Um codificador GULOSO: em cada posicao escolhe o melhor que acha agora --
//! repeticao de uma das quatro distancias recentes, casamento novo pela cadeia
//! de dispersao, repeticao curta de um byte ou literal -- e segue. O do 7-Zip e
//! OTIMO: testa ate 2^11 caminhos por posicao (`kNumOpts`, `C/LzmaEnc.c:279`)
//! com o custo de cada simbolo em bits, e comprime mais. A divergencia e
//! decisao, pelas nossas restricoes: a crate roda em microcontrolador, e o
//! otimo custa memoria e tempo que um ESP32 nao tem para gravar um
//! `config.json`. O formato e o mesmo -- qualquer decodificador LZMA2 le o que
//! sai daqui, e a prova e o 7-Zip e o liblzma lendo (`tests/phz.rs`).
//!
//! # As tres escolhas que o formato deixa
//!
//! * **Parametros lc=3, lp=0, pb=2** -- o padrao do 7-Zip (`0x5D`).
//! * **Dicionario do tamanho do conteudo**, arredondado para o valor do LZMA2
//!   logo acima e limitado a 64 MiB: quem decodifica pode reservar o
//!   dicionario declarado, e declarar 64 MiB para um JSON de 3 KiB obrigaria
//!   um decodificador de microcontrolador a recusar.
//! * **Pedaco cru quando comprimir nao paga** (o `packSize + 2 >= unpackSize`
//!   do `C/Lzma2Enc.c:153`). Onde diverge: o 7-Zip GUARDA o estado do modelo
//!   antes de tentar e o RESTAURA quando desiste (`LzmaEnc_SaveState`), e o
//!   pedaco seguinte continua sem reinicio; aqui o pedaco seguinte pede
//!   REINICIO DE ESTADO (controle 0xA0), e o modelo recomeca. Custa um pouco de
//!   compressao depois de um trecho que nao comprime, e poupa uma copia inteira
//!   do modelo -- 28 KiB, no microcontrolador.

use alloc::vec::Vec;

use crate::lzma::{
    dic_do_lzma2, tamanho_do_dic, Comprimento, Modelo, Parametros, BITS_DE_MOVIMENTO,
    BITS_DO_MODELO, FIM_DO_MODELO_DE_POSICAO, POS_BITS_MAX, TOPO,
};

/// Os parametros que o codificador usa: `(pb*5 + lp)*9 + lc` = 0x5D.
const PARAMETROS: Parametros = Parametros {
    lc: 3,
    lp: 0,
    pb: 2,
};
const BYTE_DOS_PARAMETROS: u8 = 0x5D;
/// Um pedaco LZMA do LZMA2 desempacota ate 2 MiB (`C/Lzma2Enc.c:29`) ...
const DESEMPACOTADO_MAX: usize = 1 << 21;
/// ... e empacota ate 64 KiB (`C/Lzma2Enc.c:27`). O pedaco cru tambem.
const EMPACOTADO_MAX: usize = 1 << 16;
/// Folga para o proximo simbolo e o fecho do codificador de faixa: um simbolo
/// gasta no maximo uns 60 bits, e o fecho escreve 5 bytes.
const FOLGA: usize = 64;
const CASAMENTO_MIN: usize = 2;
const CASAMENTO_MAX: usize = 273;
/// Quantas posicoes anteriores a cadeia visita: e o que troca tempo por
/// compressao.
const PROFUNDIDADE: usize = 48;
/// A propriedade de dicionario mais alta que o codificador declara: 64 MiB.
const PROP_MAXIMA: u8 = 28;

/// A menor propriedade de dicionario do LZMA2 que cobre `tamanho` bytes, ate
/// 64 MiB.
pub fn propriedade_do_dicionario(tamanho: usize) -> u8 {
    (0..PROP_MAXIMA)
        .find(|&p| {
            dic_do_lzma2(p)
                .map(|d| d as usize >= tamanho)
                .unwrap_or(false)
        })
        .unwrap_or(PROP_MAXIMA)
}

// ------------------------------------------------------ codificador de faixa

/// O codificador de faixa: o espelho do `Faixa` do decodificador.
struct Faixa {
    baixo: u64,
    alcance: u32,
    cache: u8,
    /// Quantos bytes estao esperando para sair: o `cache` e os 0xFF que um
    /// transporte ainda pode virar em 0x00.
    pendentes: usize,
    saida: Vec<u8>,
}

impl Faixa {
    fn nova() -> Faixa {
        Faixa {
            baixo: 0,
            alcance: u32::MAX,
            cache: 0,
            pendentes: 1,
            saida: Vec::new(),
        }
    }

    /// O `ShiftLow` do LZMA SDK: o byte alto de `baixo` so sai quando se sabe
    /// que nao vai mais receber transporte.
    fn deslocar(&mut self) {
        if (self.baixo as u32) < 0xFF00_0000 || (self.baixo >> 32) != 0 {
            let transporte = (self.baixo >> 32) as u8;
            let mut byte = self.cache;
            while self.pendentes > 0 {
                self.saida.push(byte.wrapping_add(transporte));
                byte = 0xFF;
                self.pendentes -= 1;
            }
            self.cache = (self.baixo >> 24) as u8;
        }
        self.pendentes += 1;
        self.baixo = ((self.baixo as u32) << 8) as u64;
    }

    fn bit(&mut self, p: &mut u16, bit: u32) {
        let v = *p as u32;
        let limite = (self.alcance >> BITS_DO_MODELO) * v;
        if bit == 0 {
            self.alcance = limite;
            *p = (v + (((1 << BITS_DO_MODELO) - v) >> BITS_DE_MOVIMENTO)) as u16;
        } else {
            self.baixo += limite as u64;
            self.alcance -= limite;
            *p = (v - (v >> BITS_DE_MOVIMENTO)) as u16;
        }
        while self.alcance < TOPO {
            self.alcance <<= 8;
            self.deslocar();
        }
    }

    fn diretos(&mut self, valor: u32, n: u32) {
        for i in (0..n).rev() {
            self.alcance >>= 1;
            if (valor >> i) & 1 == 1 {
                self.baixo += self.alcance as u64;
            }
            while self.alcance < TOPO {
                self.alcance <<= 8;
                self.deslocar();
            }
        }
    }

    /// Quanto o pedaco ocuparia se fechasse agora.
    fn tamanho(&self) -> usize {
        self.saida.len() + self.pendentes + 5
    }

    fn terminar(mut self) -> Vec<u8> {
        for _ in 0..5 {
            self.deslocar();
        }
        self.saida
    }
}

fn arvore(f: &mut Faixa, probs: &mut [u16], bits: u32, valor: u32) {
    let mut m = 1usize;
    for i in (0..bits).rev() {
        let b = (valor >> i) & 1;
        f.bit(&mut probs[m], b);
        m = (m << 1) | b as usize;
    }
}

fn arvore_reversa(f: &mut Faixa, probs: &mut [u16], bits: u32, valor: u32) {
    let mut m = 1usize;
    for i in 0..bits {
        let b = (valor >> i) & 1;
        f.bit(&mut probs[m], b);
        m = (m << 1) | b as usize;
    }
}

fn comprimento(c: &mut Comprimento, f: &mut Faixa, len0: u32, pos_estado: usize) {
    if len0 < 8 {
        f.bit(&mut c.escolha, 0);
        arvore(f, &mut c.baixo[pos_estado], 3, len0);
    } else if len0 < 16 {
        f.bit(&mut c.escolha, 1);
        f.bit(&mut c.escolha2, 0);
        arvore(f, &mut c.medio[pos_estado], 3, len0 - 8);
    } else {
        f.bit(&mut c.escolha, 1);
        f.bit(&mut c.escolha2, 1);
        arvore(f, &mut c.alto, 8, len0 - 16);
    }
}

/// A fenda de uma distancia: os dois bits mais altos dizem a fenda, o resto
/// vai pela arvore reversa ou direto (a tabela de `DOC/lzma.txt`).
fn fenda_de(dist: u32) -> u32 {
    if dist < 4 {
        return dist;
    }
    let alto = 31 - dist.leading_zeros();
    (alto << 1) | ((dist >> (alto - 1)) & 1)
}

fn distancia(m: &mut Modelo, f: &mut Faixa, dist: u32, len0: u32) {
    let estado_do_comp = (len0 as usize).min(3);
    let fenda = fenda_de(dist);
    arvore(f, &mut m.fenda[estado_do_comp], 6, fenda);
    if fenda < 4 {
        return;
    }
    let diretos = (fenda >> 1) - 1;
    let base = (2 | (fenda & 1)) << diretos;
    let resto = dist - base;
    if fenda < FIM_DO_MODELO_DE_POSICAO {
        arvore_reversa(
            f,
            &mut m.especiais[(base - fenda) as usize..],
            diretos,
            resto,
        );
    } else {
        f.diretos(resto >> 4, diretos - 4);
        arvore_reversa(f, &mut m.alinhamento, 4, resto & 0xF);
    }
}

// ------------------------------------------------------ busca de casamento

/// Cadeia de dispersao sobre tres bytes: `cabeca[h]` e a ultima posicao com
/// aquele hash, `anterior[p]` a posicao antes dela com o mesmo hash.
///
/// A memoria e a conta que decide no microcontrolador, e por isso a tabela
/// cresce com a entrada: 4 bytes por byte de entrada na cadeia, e a cabeca vai
/// de 1 KiB a 256 KiB.
struct Busca {
    cabeca: Vec<u32>,
    anterior: Vec<u32>,
    mascara: usize,
}

const NADA: u32 = u32::MAX;

impl Busca {
    fn nova(tamanho: usize) -> Busca {
        // Um balde para cada dois bytes de entrada basta para a cadeia achar os
        // casamentos. Medido num JSON de configuracao: 7.843 -> 633 bytes e
        // 160.893 -> 4.561 bytes, IGUAIS com um balde por byte e com um por
        // dois; a cabeca de um JSON de 10 KiB cai de 64 KiB para 32 KiB -- a
        // diferenca que um ESP32-C3, com 400 KiB de RAM, sente.
        let baldes = (tamanho / 2).next_power_of_two().clamp(1 << 8, 1 << 16);
        Busca {
            cabeca: alloc::vec![NADA; baldes],
            anterior: alloc::vec![NADA; tamanho],
            mascara: baldes - 1,
        }
    }

    fn hash(&self, d: &[u8], p: usize) -> usize {
        let v = (d[p] as u32) | (d[p + 1] as u32) << 8 | (d[p + 2] as u32) << 16;
        (v.wrapping_mul(0x9E37_79B1) >> 16) as usize & self.mascara
    }

    fn inserir(&mut self, d: &[u8], p: usize) {
        if p + 3 <= d.len() {
            let h = self.hash(d, p);
            self.anterior[p] = self.cabeca[h];
            self.cabeca[h] = p as u32;
        }
    }

    /// O casamento mais longo em `p`, com distancia (a partir de zero) menor
    /// que `dic`. Devolve (comprimento, distancia).
    fn melhor(&self, d: &[u8], p: usize, dic: usize) -> (usize, u32) {
        if p + 3 > d.len() {
            return (0, 0);
        }
        let teto = CASAMENTO_MAX.min(d.len() - p);
        let mut melhor = (0usize, 0u32);
        let mut cand = self.cabeca[self.hash(d, p)];
        let mut voltas = 0;
        while cand != NADA && voltas < PROFUNDIDADE {
            let c = cand as usize;
            let dist = p - c - 1;
            if dist >= dic {
                break;
            }
            let tam = comum(d, c, p, teto);
            if tam > melhor.0 {
                melhor = (tam, dist as u32);
                if tam == teto {
                    break;
                }
            }
            cand = self.anterior[c];
            voltas += 1;
        }
        melhor
    }
}

fn comum(d: &[u8], a: usize, b: usize, teto: usize) -> usize {
    let mut n = 0;
    while n < teto && d[a + n] == d[b + n] {
        n += 1;
    }
    n
}

// ------------------------------------------------------------- os simbolos

enum Simbolo {
    Literal,
    RepCurta,
    Rep { indice: usize, tam: usize },
    Casamento { dist: u32, tam: usize },
}

impl Simbolo {
    fn tamanho(&self) -> usize {
        match self {
            Simbolo::Literal | Simbolo::RepCurta => 1,
            Simbolo::Rep { tam, .. } | Simbolo::Casamento { tam, .. } => *tam,
        }
    }
}

/// A escolha gulosa. As regras sao poucas e o motivo de cada uma e o custo em
/// bits: repeticao de distancia recente e barata (a distancia nao se escreve),
/// entao ela ganha de um casamento novo so um byte maior; casamento de 3 com
/// distancia longa custa mais que tres literais bem previstos, e fica de fora.
fn escolher(m: &Modelo, busca: &Busca, d: &[u8], p: usize, dic: usize) -> Simbolo {
    let teto = CASAMENTO_MAX.min(d.len() - p);
    let mut rep = (0usize, 0usize);
    for (k, &r) in m.reps.iter().enumerate() {
        let r = r as usize;
        if r < p && r < dic && teto >= CASAMENTO_MIN {
            let tam = comum(d, p - r - 1, p, teto);
            if tam > rep.1 {
                rep = (k, tam);
            }
        }
    }
    let (tam, dist) = busca.melhor(d, p, dic);
    if rep.1 >= CASAMENTO_MIN && rep.1 + 1 >= tam {
        return Simbolo::Rep {
            indice: rep.0,
            tam: rep.1,
        };
    }
    if tam >= 4 || (tam == 3 && dist < (1 << 12)) {
        return Simbolo::Casamento { dist, tam };
    }
    let r0 = m.reps[0] as usize;
    if r0 < p && d[p - r0 - 1] == d[p] {
        return Simbolo::RepCurta;
    }
    Simbolo::Literal
}

fn codificar(m: &mut Modelo, f: &mut Faixa, d: &[u8], p: usize, s: &Simbolo) {
    let pos_estado = p & ((1 << m.par.pb) - 1);
    let s2 = (m.estado << POS_BITS_MAX) + pos_estado;
    match *s {
        Simbolo::Literal => {
            f.bit(&mut m.e_casamento[s2], 0);
            let anterior = if p > 0 { d[p - 1] } else { 0 };
            let ctx =
                ((p & ((1 << m.par.lp) - 1)) << m.par.lc) + ((anterior as usize) >> (8 - m.par.lc));
            let probs = &mut m.literais[0x300 * ctx..0x300 * (ctx + 1)];
            let byte = d[p] as usize;
            let mut simbolo = 1usize;
            let mut casando = m.estado >= 7;
            let casado = if casando {
                d[p - m.reps[0] as usize - 1] as usize
            } else {
                0
            };
            for i in (0..8).rev() {
                let bit = (byte >> i) & 1;
                if casando {
                    let bit_casado = (casado >> i) & 1;
                    f.bit(&mut probs[((1 + bit_casado) << 8) + simbolo], bit as u32);
                    casando = bit_casado == bit;
                } else {
                    f.bit(&mut probs[simbolo], bit as u32);
                }
                simbolo = (simbolo << 1) | bit;
            }
            m.estado = match m.estado {
                0..=3 => 0,
                4..=9 => m.estado - 3,
                _ => m.estado - 6,
            };
        }
        Simbolo::RepCurta => {
            f.bit(&mut m.e_casamento[s2], 1);
            f.bit(&mut m.e_rep[m.estado], 1);
            f.bit(&mut m.e_rep_g0[m.estado], 0);
            f.bit(&mut m.e_rep0_longo[s2], 0);
            m.estado = if m.estado < 7 { 9 } else { 11 };
        }
        Simbolo::Rep { indice, tam } => {
            f.bit(&mut m.e_casamento[s2], 1);
            f.bit(&mut m.e_rep[m.estado], 1);
            if indice == 0 {
                f.bit(&mut m.e_rep_g0[m.estado], 0);
                f.bit(&mut m.e_rep0_longo[s2], 1);
            } else {
                f.bit(&mut m.e_rep_g0[m.estado], 1);
                if indice == 1 {
                    f.bit(&mut m.e_rep_g1[m.estado], 0);
                } else {
                    f.bit(&mut m.e_rep_g1[m.estado], 1);
                    f.bit(&mut m.e_rep_g2[m.estado], (indice == 3) as u32);
                }
                let dist = m.reps[indice];
                for j in (1..=indice).rev() {
                    m.reps[j] = m.reps[j - 1];
                }
                m.reps[0] = dist;
            }
            comprimento(&mut m.comp_rep, f, (tam - 2) as u32, pos_estado);
            m.estado = if m.estado < 7 { 8 } else { 11 };
        }
        Simbolo::Casamento { dist, tam } => {
            f.bit(&mut m.e_casamento[s2], 1);
            f.bit(&mut m.e_rep[m.estado], 0);
            let len0 = (tam - 2) as u32;
            comprimento(&mut m.comp, f, len0, pos_estado);
            m.estado = if m.estado < 7 { 7 } else { 10 };
            distancia(m, f, dist, len0);
            m.reps = [dist, m.reps[0], m.reps[1], m.reps[2]];
        }
    }
}

// --------------------------------------------------------------- o LZMA2

fn cabecalho_lzma(saida: &mut Vec<u8>, controle: u8, desempacotado: usize, empacotado: usize) {
    let u = desempacotado - 1;
    let e = empacotado - 1;
    saida.push(controle | ((u >> 16) as u8 & 0x1F));
    saida.extend_from_slice(&[(u >> 8) as u8, u as u8, (e >> 8) as u8, e as u8]);
    if controle >= 0xC0 {
        saida.push(BYTE_DOS_PARAMETROS);
    }
}

/// Comprime `dados` num fluxo LZMA2 completo, com o dicionario da
/// propriedade `prop` (que vai no coder do 7z).
pub fn lzma2(dados: &[u8], prop: u8) -> Vec<u8> {
    let mut saida = Vec::new();
    // Entrada que nao cabe nas posicoes de 32 bits da cadeia vai inteira em
    // pedacos crus: continua LZMA2 valido, so nao comprime.
    let so_cru = u32::try_from(dados.len())
        .map(|n| n == u32::MAX)
        .unwrap_or(true);
    let dic = tamanho_do_dic(dic_do_lzma2(prop).unwrap_or(u32::MAX));
    let mut m = Modelo::novo(PARAMETROS);
    let mut busca = Busca::nova(if so_cru { 0 } else { dados.len() });
    let mut p = 0usize;
    // O que o proximo pedaco LZMA tem de reiniciar, pela regra do decodificador
    // (`needInitLevel`): tudo no comeco, parametros depois de um cru que
    // reiniciou o dicionario, estado depois de um cru que interrompeu o modelo.
    let mut primeiro = true;
    let mut exige_parametros = false;
    let mut exige_estado = false;
    while p < dados.len() {
        let inicio = p;
        let mut f = Faixa::nova();
        if !so_cru {
            while p < dados.len()
                && p - inicio + CASAMENTO_MAX <= DESEMPACOTADO_MAX
                && f.tamanho() + FOLGA <= EMPACOTADO_MAX
            {
                let s = escolher(&m, &busca, dados, p, dic);
                codificar(&mut m, &mut f, dados, p, &s);
                for q in p..p + s.tamanho() {
                    busca.inserir(dados, q);
                }
                p += s.tamanho();
            }
        } else {
            p = (p + EMPACOTADO_MAX).min(dados.len());
        }
        let desempacotado = p - inicio;
        let empacotado = if so_cru { Vec::new() } else { f.terminar() };
        if !so_cru && empacotado.len() + 2 < desempacotado {
            let controle = if primeiro {
                0xE0
            } else if exige_parametros {
                0xC0
            } else if exige_estado {
                0xA0
            } else {
                0x80
            };
            cabecalho_lzma(&mut saida, controle, desempacotado, empacotado.len());
            saida.extend_from_slice(&empacotado);
            primeiro = false;
            exige_parametros = false;
            exige_estado = false;
        } else {
            for pedaco in dados[inicio..p].chunks(EMPACOTADO_MAX) {
                let u = pedaco.len() - 1;
                saida.extend_from_slice(&[
                    if primeiro { 0x01 } else { 0x02 },
                    (u >> 8) as u8,
                    u as u8,
                ]);
                saida.extend_from_slice(pedaco);
                if primeiro {
                    primeiro = false;
                    exige_parametros = true;
                }
            }
            exige_estado = true;
            m = Modelo::novo(PARAMETROS);
        }
    }
    saida.push(0x00);
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::lzma;
    use alloc::format;
    use alloc::string::String;

    fn xorshift(n: usize, mut x: u32) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            v.push(x as u8);
        }
        v
    }

    fn json_de_mentira(linhas: usize) -> Vec<u8> {
        let mut t = String::from("{\n");
        for i in 0..linhas {
            t.push_str(&format!(
                "  \"chave_{i}\": {{\"porta\": {}, \"nome\": \"servico {}\", \"ligado\": {}}},\n",
                5000 + i % 17,
                i % 5,
                i % 2 == 0
            ));
        }
        t.push('}');
        t.into_bytes()
    }

    fn ida_e_volta(dados: &[u8]) -> Vec<u8> {
        let prop = propriedade_do_dicionario(dados.len());
        let c = lzma2(dados, prop);
        let volta = lzma::lzma2(prop, &c, dados.len()).expect("o proprio fluxo nao voltou");
        assert_eq!(volta, dados, "voltou diferente");
        c
    }

    #[test]
    fn a_fenda_segue_a_tabela() {
        let casos = [
            (0, 0),
            (3, 3),
            (4, 4),
            (5, 4),
            (6, 5),
            (7, 5),
            (8, 6),
            (12, 7),
            (16, 8),
        ];
        for (dist, fenda) in casos {
            assert_eq!(fenda_de(dist), fenda, "distancia {dist}");
        }
        assert_eq!(fenda_de(u32::MAX), 63);
    }

    #[test]
    fn vai_e_volta_em_varios_formatos() {
        ida_e_volta(b"");
        ida_e_volta(b"a");
        ida_e_volta(b"ab");
        ida_e_volta(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        ida_e_volta(&(0..=255u8).collect::<Vec<u8>>());
        ida_e_volta(&xorshift(5000, 7));
    }

    /// O caso para o qual a compressao existe: texto de configuracao.
    #[test]
    fn comprime_json_de_verdade() {
        let j = json_de_mentira(400);
        let c = ida_e_volta(&j);
        assert!(
            c.len() * 5 < j.len(),
            "{} bytes viraram {} -- menos de 5x",
            j.len(),
            c.len()
        );
    }

    /// Ruido nao comprime: sai em pedacos crus, e maior so pelos cabecalhos.
    #[test]
    fn ruido_vai_cru() {
        let r = xorshift(200_000, 99);
        let c = ida_e_volta(&r);
        assert_eq!(c[0], 0x01, "o primeiro pedaco devia ser cru com reinicio");
        assert!(c.len() <= r.len() + 3 * 4 + 1);
    }

    /// Mais de 2 MiB compressivel: varios pedacos LZMA, o segundo em diante
    /// SEM reinicio -- o estado atravessa a fronteira dos dois lados.
    #[test]
    fn varios_pedacos_continuam_o_estado() {
        let mut d = Vec::new();
        while d.len() < 5_000_000 {
            d.extend_from_slice(&json_de_mentira(50));
        }
        let c = ida_e_volta(&d);
        assert_eq!(c[0], 0xE0 | (c[0] & 0x1f));
        assert!(c.len() * 20 < d.len());
    }

    /// Ruido seguido de texto: o cru reinicia o dicionario, e o LZMA seguinte
    /// tem de vir com parametros (0xC0) -- a regra do decodificador.
    #[test]
    fn cru_seguido_de_texto() {
        let mut d = xorshift(150_000, 3);
        d.extend_from_slice(&json_de_mentira(3000));
        d.extend_from_slice(&xorshift(70_000, 5));
        d.extend_from_slice(&json_de_mentira(3000));
        ida_e_volta(&d);
    }

    #[test]
    fn dicionario_do_tamanho_do_conteudo() {
        assert_eq!(propriedade_do_dicionario(0), 0);
        assert_eq!(propriedade_do_dicionario(4096), 0);
        assert_eq!(propriedade_do_dicionario(4097), 1);
        assert_eq!(propriedade_do_dicionario(65_536), 8);
        assert_eq!(propriedade_do_dicionario(usize::MAX), PROP_MAXIMA);
    }
}
