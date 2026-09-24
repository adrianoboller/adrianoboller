//! Codificador LZMA2 -- o que faz o PhxZip comprimir, e nao so empacotar.
//!
//! # O que ele faz
//!
//! Busca por cadeia de dispersao de 4 bytes e as quatro distancias repetidas
//! conferidas a cada posicao. Do nivel 5 em diante a escolha e por PRECO em
//! bits, caminho minimo sobre os simbolos possiveis (`otimo.rs`); do 1 ao 4,
//! gulosa com um passo de preguica.
//!
//! Medido contra o `7z -mf=off` (`docs/PHXZIP.md` §3): +3,6% de tamanho no
//! nivel 5 e +2,0% no 9. O que falta para empatar e a arvore binaria de
//! busca (`bt4`) e os casamentos de 2 bytes por dispersao -- nao medidos.
//!
//! # Os pedacos do LZMA2
//!
//! Cada pedaco comprimido fecha antes de 64 KiB comprimidos e de 2 MiB
//! descomprimidos (os dois campos do cabecalho do pedaco). O que nao
//! comprimiu vira pedaco CRU, e o pedaco LZMA seguinte reinicia o estado --
//! e o mesmo que o `xz` faz, e e o que impede dado ja comprimido de crescer.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use super::faixa::Codificador;
use super::modelo::{
    estado_de_compr, Comprimentos, Modelo, COMPR_MAX, COMPR_MIN, FIM_DO_MODELO_DE_POS,
};
use super::otimo::Otimo;
use super::{byte_lzma2, Props};

const PEDACO_DESC_MAX: usize = 1 << 21;
const PEDACO_COMP_MAX: usize = 1 << 16;
const PEDACO_CRU_MAX: usize = 1 << 16;
/// Folga para o maior simbolo possivel (casamento longo com distancia longa):
/// bem abaixo de 32 bytes, mesmo com as probabilidades mais desfavoraveis.
const FOLGA: usize = 32;

/// Quanto se procura: dicionario, profundidade da cadeia, «bom o bastante».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nivel {
    /// Tamanho do dicionario, em bytes (potencia de 2).
    pub dicionario: u32,
    /// Quantos candidatos da cadeia se conferem por posicao.
    pub profundidade: u32,
    /// Casamento deste tamanho encerra a busca.
    pub bom: usize,
    /// Analise preguicosa de um passo.
    pub preguicoso: bool,
    /// Analise otima por preco (o `GetOptimum` do 7-Zip, reescrito aqui).
    pub otimo: bool,
}

impl Nivel {
    /// Nivel de 1 (rapido) a 9 (maximo), na escala do `7z -mx`. Fora da faixa
    /// satura.
    pub fn de(n: u8) -> Nivel {
        // Do 5 em diante a escolha e por preco em bits, e o «bom» e a
        // profundidade seguem o 7-Zip (fb e mc): o preco faz a busca render.
        let (log_dic, profundidade, bom, preguicoso, otimo) = match n {
            0 | 1 => (16, 4, 16, false, false),
            2 => (18, 8, 24, false, false),
            3 => (20, 12, 32, true, false),
            4 => (22, 16, 48, true, false),
            5 => (24, 32, 32, true, true),
            6 => (24, 48, 64, true, true),
            7 => (25, 64, 64, true, true),
            8 => (26, 96, 128, true, true),
            _ => (26, 128, COMPR_MAX, true, true),
        };
        Nivel {
            dicionario: 1 << log_dic,
            profundidade,
            bom,
            preguicoso,
            otimo,
        }
    }
}

pub(super) struct Buscador<'a> {
    dados: &'a [u8],
    cabeca: Vec<u32>,
    anterior: Vec<u32>,
    mascara: usize,
    bits: u32,
    janela: usize,
    profundidade: u32,
    bom: usize,
    /// Proxima posicao a entrar na cadeia. A insercao e monotona: cada
    /// posicao entra uma vez, na ordem -- quem pula um trecho (um casamento)
    /// nao precisa lembrar de inserir o que pulou, e quem volta atras (o
    /// planejador que descartou um plano) nao insere duas vezes.
    proximo: usize,
}

impl<'a> Buscador<'a> {
    fn novo(dados: &'a [u8], janela: usize, nivel: &Nivel) -> Buscador<'a> {
        let bits = if dados.len() < (1 << 16) { 14 } else { 18 };
        Buscador {
            dados,
            cabeca: vec![0; 1 << bits],
            anterior: vec![0; janela],
            mascara: janela - 1,
            bits,
            janela,
            profundidade: nivel.profundidade,
            bom: nivel.bom,
            proximo: 0,
        }
    }

    #[inline]
    fn dispersao(&self, p: usize) -> usize {
        let d = self.dados;
        let v = u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - self.bits)) as usize
    }

    fn inserir_proximo(&mut self) {
        let p = self.proximo;
        self.proximo += 1;
        if p + 4 > self.dados.len() {
            return;
        }
        let h = self.dispersao(p);
        self.anterior[p & self.mascara] = self.cabeca[h];
        self.cabeca[h] = p as u32 + 1;
    }

    /// Todos os casamentos em `p` que melhoram o comprimento, do mais curto
    /// (e mais perto) ao mais longo: para cada comprimento, a primeira
    /// entrada que o alcanca tem a menor distancia -- e o que o preco quer.
    pub(super) fn todos(&mut self, p: usize, saida: &mut Vec<(usize, usize)>) {
        saida.clear();
        while self.proximo < p {
            self.inserir_proximo();
        }
        let d = self.dados;
        if p + 4 > d.len() {
            if self.proximo == p {
                self.proximo += 1;
            }
            return;
        }
        let h = self.dispersao(p);
        let mut cand = self.cabeca[h];
        if self.proximo == p {
            self.inserir_proximo();
        }
        let max = (d.len() - p).min(COMPR_MAX);
        let mut melhor = 0usize;
        let mut resta = self.profundidade;
        while cand != 0 && resta > 0 {
            let c = cand as usize - 1;
            let prox = self.anterior[c & self.mascara];
            // Posicao ja inserida adiante de `p` (plano descartado): pula.
            if c < p {
                let dist = p - c;
                if dist > self.janela {
                    break;
                }
                let k = melhor.min(max - 1);
                if d[c + k] == d[p + k] {
                    let mut n = 0;
                    while n < max && d[c + n] == d[p + n] {
                        n += 1;
                    }
                    if n > melhor {
                        melhor = n;
                        saida.push((n, dist));
                        if n >= self.bom || n == max {
                            break;
                        }
                    }
                }
                resta -= 1;
            }
            // A janela circular sobrescreve: candidato que nao e mais antigo
            // que o atual e posicao nova no mesmo slot, e a cadeia acabou.
            if prox >= cand {
                break;
            }
            cand = prox;
        }
    }
}

pub(super) fn compr_comum(d: &[u8], p: usize, dist: usize, max: usize) -> usize {
    let mut n = 0;
    while n < max && d[p + n] == d[p + n - dist] {
        n += 1;
    }
    n
}

fn gravar_compr(rc: &mut Codificador, c: &mut Comprimentos, pe: usize, compr: usize) {
    let l = (compr - COMPR_MIN) as u32;
    if l < 8 {
        rc.bit(&mut c.escolha, 0);
        rc.arvore(&mut c.baixo[pe], 3, l);
    } else if l < 16 {
        rc.bit(&mut c.escolha, 1);
        rc.bit(&mut c.escolha2, 0);
        rc.arvore(&mut c.meio[pe], 3, l - 8);
    } else {
        rc.bit(&mut c.escolha, 1);
        rc.bit(&mut c.escolha2, 1);
        rc.arvore(&mut c.alto, 8, l - 16);
    }
}

pub(super) struct Escrita<'a> {
    pub d: &'a [u8],
    pub m: Modelo,
}

impl Escrita<'_> {
    fn literal(&mut self, rc: &mut Codificador, p: usize) {
        let m = &mut self.m;
        let pe = m.pos_estado(p as u64);
        rc.bit(&mut m.e_casamento[m.estado][pe], 0);
        let anterior = if p > 0 { self.d[p - 1] } else { 0 };
        let base = m.base_literal(p as u64, anterior);
        let probs = &mut m.literais[base..base + 0x300];
        let byte = self.d[p] as usize;
        let mut s = 1usize;
        let mut casado = m.estado >= 7;
        let alvo = if casado {
            self.d[p - m.reps[0] as usize - 1] as usize
        } else {
            0
        };
        for i in (0..8).rev() {
            let b = (byte >> i) & 1;
            if casado {
                let bc = (alvo >> i) & 1;
                rc.bit(&mut probs[0x100 + (bc << 8) + s], b as u32);
                casado = bc == b;
            } else {
                rc.bit(&mut probs[s], b as u32);
            }
            s = (s << 1) | b;
        }
        m.apos_literal();
    }

    fn casamento(&mut self, rc: &mut Codificador, p: usize, compr: usize, dist: usize) {
        let m = &mut self.m;
        let pe = m.pos_estado(p as u64);
        rc.bit(&mut m.e_casamento[m.estado][pe], 1);
        rc.bit(&mut m.e_rep[m.estado], 0);
        gravar_compr(rc, &mut m.compr, pe, compr);
        let v = (dist - 1) as u32;
        let fenda = if v < 4 {
            v
        } else {
            let n = 31 - v.leading_zeros();
            (n << 1) | ((v >> (n - 1)) & 1)
        };
        rc.arvore(&mut m.fenda[estado_de_compr(compr)], 6, fenda);
        if fenda >= 4 {
            let n = (fenda >> 1) - 1;
            let base = (2 | (fenda & 1)) << n;
            let resto = v - base;
            if fenda < FIM_DO_MODELO_DE_POS {
                let i = (base - fenda) as usize;
                rc.arvore_reversa(&mut m.especial[i..], n, resto);
            } else {
                rc.diretos(resto >> 4, n - 4);
                rc.arvore_reversa(&mut m.alinhamento, 4, resto & 15);
            }
        }
        m.reps = [v, m.reps[0], m.reps[1], m.reps[2]];
        m.apos_casamento();
    }

    fn rep(&mut self, rc: &mut Codificador, p: usize, indice: usize, compr: usize) {
        let m = &mut self.m;
        let pe = m.pos_estado(p as u64);
        let st = m.estado;
        rc.bit(&mut m.e_casamento[st][pe], 1);
        rc.bit(&mut m.e_rep[st], 1);
        if indice == 0 {
            rc.bit(&mut m.e_rep_g0[st], 0);
            rc.bit(&mut m.e_rep0_longo[st][pe], 1);
        } else {
            rc.bit(&mut m.e_rep_g0[st], 1);
            if indice == 1 {
                rc.bit(&mut m.e_rep_g1[st], 0);
            } else {
                rc.bit(&mut m.e_rep_g1[st], 1);
                rc.bit(&mut m.e_rep_g2[st], (indice - 2) as u32);
            }
            let d = m.reps[indice];
            for k in (1..=indice).rev() {
                m.reps[k] = m.reps[k - 1];
            }
            m.reps[0] = d;
        }
        gravar_compr(rc, &mut m.compr_rep, pe, compr);
        m.apos_rep();
    }

    fn rep_curto(&mut self, rc: &mut Codificador, p: usize) {
        let m = &mut self.m;
        let pe = m.pos_estado(p as u64);
        let st = m.estado;
        rc.bit(&mut m.e_casamento[st][pe], 1);
        rc.bit(&mut m.e_rep[st], 1);
        rc.bit(&mut m.e_rep_g0[st], 0);
        rc.bit(&mut m.e_rep0_longo[st][pe], 0);
        m.apos_rep_curto();
    }

    fn melhor_rep(&self, p: usize, max: usize) -> (usize, usize) {
        let mut melhor = (0usize, 0usize);
        for (i, r) in self.m.reps.iter().enumerate() {
            let dist = *r as usize + 1;
            if dist > p {
                continue;
            }
            let n = compr_comum(self.d, p, dist, max);
            if n > melhor.0 {
                melhor = (n, i);
            }
        }
        melhor
    }
}

/// Um simbolo decidido, antes de ir para o codificador de faixa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Decisao {
    /// Um byte cru.
    Literal,
    /// Um byte repetido da distancia `reps[0]`.
    RepCurto,
    /// Repeticao de uma das quatro distancias guardadas (indice, comprimento).
    Rep(u8, u16),
    /// Casamento novo (distancia, comprimento).
    Casa(u32, u16),
}

impl Decisao {
    pub(super) fn compr(self) -> usize {
        match self {
            Decisao::Literal | Decisao::RepCurto => 1,
            Decisao::Rep(_, l) | Decisao::Casa(_, l) => l as usize,
        }
    }
}

impl Escrita<'_> {
    fn emitir(&mut self, rc: &mut Codificador, p: usize, d: Decisao) {
        match d {
            Decisao::Literal => self.literal(rc, p),
            Decisao::RepCurto => self.rep_curto(rc, p),
            Decisao::Rep(i, l) => self.rep(rc, p, i as usize, l as usize),
            Decisao::Casa(dist, l) => self.casamento(rc, p, l as usize, dist as usize),
        }
    }
}

/// A escolha gulosa (niveis 1 a 4): o maior casamento, repeticao quando ela
/// empata, e um passo de preguica.
fn planejar_guloso(
    e: &Escrita,
    busca: &mut Buscador,
    lista: &mut Vec<(usize, usize)>,
    adiantado: &mut Option<(usize, usize, usize)>,
    p: usize,
    nivel: &Nivel,
    fila: &mut VecDeque<Decisao>,
) {
    let dados = e.d;
    let n = dados.len();
    let max = (n - p).min(COMPR_MAX);
    let (mc, md) = match adiantado.take() {
        Some((q, c, d)) if q == p => (c, d),
        _ => {
            busca.todos(p, lista);
            lista.last().copied().unwrap_or((0, 0))
        }
    };
    let (rc_len, rc_idx) = e.melhor_rep(p, max);
    // Casamento cuja distancia ja esta nas repetidas sai como repeticao.
    if rc_len >= COMPR_MIN && (rc_len + 1 >= mc || rc_len >= nivel.bom) {
        fila.push_back(Decisao::Rep(rc_idx as u8, rc_len as u16));
        return;
    }
    // Casamento de 3 muito longe custa mais que tres literais.
    if mc >= 4 || (mc == 3 && md <= 1 << 12) {
        if nivel.preguicoso && mc < nivel.bom && p + 1 < n {
            busca.todos(p + 1, lista);
            let (nc, nd) = lista.last().copied().unwrap_or((0, 0));
            if nc > mc + usize::from(nd > md.saturating_mul(8)) {
                fila.push_back(Decisao::Literal);
                *adiantado = Some((p + 1, nc, nd));
                return;
            }
        }
        fila.push_back(Decisao::Casa(md as u32, mc as u16));
        return;
    }
    let d0 = e.m.reps[0] as usize + 1;
    if d0 <= p && dados[p] == dados[p - d0] && e.m.estado >= 7 {
        fila.push_back(Decisao::RepCurto);
    } else {
        fila.push_back(Decisao::Literal);
    }
}

/// Comprime `dados` em LZMA2. Devolve o byte de propriedade (dicionario) e o
/// fluxo, pronto para o coder `21` do 7z.
pub fn codificar_lzma2(dados: &[u8], nivel: Nivel) -> (u8, Vec<u8>) {
    let n = dados.len();
    // Dicionario maior que o dado so gasta memoria de quem descomprime.
    let janela = (nivel.dicionario as usize)
        .min(n.max(4096).next_power_of_two())
        .max(4096);
    let prop = byte_lzma2(janela as u32);
    let props = Props::PADRAO;
    let mut saida = Vec::with_capacity(n / 2 + 16);
    let mut busca = Buscador::novo(dados, janela, &nivel);
    let mut e = Escrita {
        d: dados,
        m: Modelo::novo(props),
    };
    let (mut quer_dict, mut quer_props, mut quer_estado) = (true, true, true);
    let mut p = 0usize;
    let mut fila: VecDeque<Decisao> = VecDeque::new();
    let mut lista = Vec::new();
    let mut adiantado = None;
    let mut otimo = Otimo::novo();

    while p < n {
        let inicio = p;
        let mut rc = Codificador::novo();
        while p < n
            && p - inicio + COMPR_MAX <= PEDACO_DESC_MAX
            && rc.tamanho_ao_terminar() + FOLGA <= PEDACO_COMP_MAX
        {
            if fila.is_empty() {
                if nivel.otimo {
                    otimo.planejar(&e.m, dados, &mut busca, p, nivel.bom, &mut fila);
                } else {
                    planejar_guloso(
                        &e,
                        &mut busca,
                        &mut lista,
                        &mut adiantado,
                        p,
                        &nivel,
                        &mut fila,
                    );
                }
            }
            let Some(d) = fila.pop_front() else { break };
            e.emitir(&mut rc, p, d);
            p += d.compr();
        }
        let comp = rc.terminar();
        let desc = p - inicio;
        if comp.len() > PEDACO_COMP_MAX || comp.len() >= desc {
            for fatia in dados[inicio..p].chunks(PEDACO_CRU_MAX) {
                saida.push(if quer_dict { 1 } else { 2 });
                quer_dict = false;
                saida.extend_from_slice(&((fatia.len() - 1) as u16).to_be_bytes());
                saida.extend_from_slice(fatia);
            }
            // O decodificador nao viu os simbolos deste pedaco: o proximo
            // pedaco LZMA recomeca do estado inicial, dos dois lados. E o
            // plano pendente morre junto -- uma repeticao planejada com as
            // distancias de antes apontaria para outro lugar depois do reinicio.
            e.m.reiniciar(props);
            fila.clear();
            adiantado = None;
            quer_estado = true;
        } else {
            let modo: u8 = if quer_dict {
                3
            } else if quer_props {
                2
            } else if quer_estado {
                1
            } else {
                0
            };
            saida.push(0x80 | (modo << 5) | ((desc - 1) >> 16) as u8);
            saida.extend_from_slice(&(((desc - 1) & 0xFFFF) as u16).to_be_bytes());
            saida.extend_from_slice(&((comp.len() - 1) as u16).to_be_bytes());
            if modo >= 2 {
                saida.push(props.para_byte());
            }
            saida.extend_from_slice(&comp);
            quer_dict = false;
            quer_props = false;
            quer_estado = false;
        }
    }
    saida.push(0);
    (prop, saida)
}

#[cfg(test)]
mod testes {
    use super::super::decodificar_lzma2;
    use super::*;

    fn ida_e_volta(d: &[u8], nivel: u8) -> usize {
        let (p, c) = codificar_lzma2(d, Nivel::de(nivel));
        let mut v = Vec::new();
        decodificar_lzma2(p, &c, d.len(), &mut v).unwrap();
        assert_eq!(v, d, "nivel {nivel}, {} bytes", d.len());
        c.len()
    }

    fn pseudo(n: usize, semente: u32) -> Vec<u8> {
        let mut x = semente;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    #[test]
    fn vazio_e_um_byte() {
        assert_eq!(ida_e_volta(b"", 5), 1);
        ida_e_volta(b"a", 5);
        ida_e_volta(b"ab", 1);
    }

    #[test]
    fn texto_repetido_comprime() {
        let t = b"O pai veio antes do filho, e o rowstamp prova. ".repeat(500);
        let c = ida_e_volta(&t, 5);
        assert!(c * 20 < t.len(), "{c} de {}", t.len());
    }

    #[test]
    fn aleatorio_nao_cresce_mais_que_o_cabecalho_dos_pedacos() {
        let d = pseudo(300_000, 7);
        let c = ida_e_volta(&d, 5);
        // Pedacos crus: 3 bytes a cada 64 KiB, mais o byte de fim.
        assert!(c <= d.len() + 3 * (d.len() / 65_536 + 1) + 1, "{c}");
    }

    #[test]
    fn todos_os_niveis_e_mistura_de_cru_com_comprimido() {
        let mut d = pseudo(70_000, 3);
        d.extend(b"abcabcabcabd".repeat(9000));
        d.extend(pseudo(5_000, 9));
        d.extend(vec![0u8; 3_000_000]);
        for nivel in [1, 5, 9] {
            ida_e_volta(&d, nivel);
        }
    }

    /// A analise por preco tem de valer o que custa: no mesmo nivel, com a
    /// mesma busca, o caminho otimo sai menor que o guloso. Se o planejador
    /// deixasse de ser chamado, este teste cai.
    #[test]
    fn o_preco_ganha_do_guloso_em_texto() {
        let mut t = Vec::new();
        for i in 0..4000u32 {
            let linha = alloc::format!(
                "| {} | pedido {} do dono | estado {} | medido em {} |\n",
                i % 97,
                i * 7 % 450,
                ["aberto", "feito", "parcial"][(i % 3) as usize],
                i % 31
            );
            t.extend_from_slice(linha.as_bytes());
        }
        let otimo = Nivel::de(5);
        let guloso = Nivel {
            otimo: false,
            ..otimo
        };
        let (_, co) = codificar_lzma2(&t, otimo);
        let (_, cg) = codificar_lzma2(&t, guloso);
        assert!(
            co.len() * 100 <= cg.len() * 98,
            "otimo {} guloso {}",
            co.len(),
            cg.len()
        );
        ida_e_volta(&t, 5);
    }

    #[test]
    fn distancias_das_quatro_faixas() {
        // Repeticoes a distancia curta, media (modelo especial) e longa
        // (bits diretos + alinhamento), para exercitar todo o codigo de distancia.
        let base = pseudo(200_000, 11);
        let mut d = Vec::new();
        for dist in [1usize, 2, 3, 5, 17, 100, 1000, 70_000, 150_000] {
            d.extend_from_slice(&base[..dist.min(base.len())]);
            let ini = d.len() - dist.min(d.len());
            let copia = d[ini..ini + 40.min(d.len() - ini)].to_vec();
            d.extend(copia);
        }
        ida_e_volta(&d, 9);
    }
}
