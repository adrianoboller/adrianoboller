//! Codificador LZMA2 -- o que faz o PhxZip comprimir, e nao so empacotar.
//!
//! # O que ele faz, e o que ainda nao faz
//!
//! Busca por cadeia de dispersao (3 bytes), as quatro distancias repetidas
//! conferidas a cada posicao, e analise preguicosa de um passo: antes de
//! emitir um casamento, olha se a posicao seguinte casa mais longe.
//!
//! **Nao** faz a analise otima por preco do 7-Zip (`LzmaEnc.c`, o
//! `GetOptimum`), que escolhe entre caminhos somando o custo em bits de cada
//! simbolo. Ela compra alguns por cento a mais de compressao ao preco de uma
//! tabela de precos e de um laco de programacao dinamica. A diferenca medida
//! contra o `7z -mx` vai no documento da crate, com o numero -- e e a proxima
//! frente do codificador, nao um defeito escondido.
//!
//! # Os pedacos do LZMA2
//!
//! Cada pedaco comprimido fecha antes de 64 KiB comprimidos e de 2 MiB
//! descomprimidos (os dois campos do cabecalho do pedaco). O que nao
//! comprimiu vira pedaco CRU, e o pedaco LZMA seguinte reinicia o estado --
//! e o mesmo que o `xz` faz, e e o que impede dado ja comprimido de crescer.

use alloc::vec;
use alloc::vec::Vec;

use super::faixa::Codificador;
use super::modelo::{
    estado_de_compr, Comprimentos, Modelo, COMPR_MAX, COMPR_MIN, FIM_DO_MODELO_DE_POS,
};
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
}

impl Nivel {
    /// Nivel de 1 (rapido) a 9 (maximo), na escala do `7z -mx`. Fora da faixa
    /// satura.
    pub fn de(n: u8) -> Nivel {
        let (log_dic, profundidade, bom, preguicoso) = match n {
            0 | 1 => (16, 4, 16, false),
            2 => (18, 8, 24, false),
            3 => (20, 12, 32, true),
            4 => (22, 16, 48, true),
            5 => (24, 32, 64, true),
            6 => (24, 64, 96, true),
            7 => (25, 128, 128, true),
            8 => (26, 256, 192, true),
            _ => (26, 1024, COMPR_MAX, true),
        };
        Nivel {
            dicionario: 1 << log_dic,
            profundidade,
            bom,
            preguicoso,
        }
    }
}

struct Buscador<'a> {
    dados: &'a [u8],
    cabeca: Vec<u32>,
    anterior: Vec<u32>,
    mascara: usize,
    bits: u32,
    janela: usize,
    profundidade: u32,
    bom: usize,
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
        }
    }

    #[inline]
    fn dispersao(&self, p: usize) -> usize {
        let d = self.dados;
        let v = (d[p] as u32) | ((d[p + 1] as u32) << 8) | ((d[p + 2] as u32) << 16);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - self.bits)) as usize
    }

    /// Insere `p` na cadeia e devolve o candidato anterior (mais 1; 0 = nada).
    #[inline]
    fn inserir(&mut self, p: usize) -> u32 {
        if p + 3 > self.dados.len() {
            return 0;
        }
        let h = self.dispersao(p);
        let c = self.cabeca[h];
        self.cabeca[h] = p as u32 + 1;
        self.anterior[p & self.mascara] = c;
        c
    }

    /// Maior casamento em `p` (comprimento, distancia), inserindo `p`.
    fn buscar(&mut self, p: usize) -> (usize, usize) {
        let mut cand = self.inserir(p);
        let d = self.dados;
        let max = (d.len() - p).min(COMPR_MAX);
        let mut melhor = (0usize, 0usize);
        if max < 3 {
            return melhor;
        }
        let mut resta = self.profundidade;
        while cand != 0 && resta > 0 {
            let c = cand as usize - 1;
            let dist = p - c;
            if dist > self.janela {
                break;
            }
            if d[c + melhor.0.min(max - 1)] == d[p + melhor.0.min(max - 1)] {
                let mut n = 0;
                while n < max && d[c + n] == d[p + n] {
                    n += 1;
                }
                if n > melhor.0 {
                    melhor = (n, dist);
                    if n >= self.bom || n == max {
                        break;
                    }
                }
            }
            let prox = self.anterior[c & self.mascara];
            // A janela circular sobrescreve: candidato que nao e mais antigo
            // que o atual e posicao nova no mesmo slot, e a cadeia acabou.
            if prox as usize >= cand as usize {
                break;
            }
            cand = prox;
            resta -= 1;
        }
        melhor
    }
}

fn compr_comum(d: &[u8], p: usize, dist: usize, max: usize) -> usize {
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

struct Escrita<'a> {
    d: &'a [u8],
    m: Modelo,
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
    let mut adiantado: Option<(usize, usize, usize)> = None;

    while p < n {
        let inicio = p;
        let mut rc = Codificador::novo();
        while p < n
            && p - inicio + COMPR_MAX <= PEDACO_DESC_MAX
            && rc.tamanho_ao_terminar() + FOLGA <= PEDACO_COMP_MAX
        {
            let max = (n - p).min(COMPR_MAX);
            let (mc, md) = match adiantado.take() {
                Some((q, c, d)) if q == p => (c, d),
                _ => busca.buscar(p),
            };
            let (rc_len, rc_idx) = e.melhor_rep(p, max);
            // Casamento cuja distancia ja esta nas repetidas sai como repeticao.
            if rc_len >= COMPR_MIN && (rc_len + 1 >= mc || rc_len >= nivel.bom) {
                e.rep(&mut rc, p, rc_idx, rc_len);
                for q in p + 1..p + rc_len {
                    busca.inserir(q);
                }
                p += rc_len;
                continue;
            }
            // Casamento de 3 muito longe custa mais que tres literais.
            let vale = mc >= 4 || (mc == 3 && md <= 1 << 12);
            if vale {
                if nivel.preguicoso && mc < nivel.bom && p + 1 < n {
                    let (nc, nd) = busca.buscar(p + 1);
                    if nc > mc + usize::from(nd > md.saturating_mul(8)) {
                        e.literal(&mut rc, p);
                        adiantado = Some((p + 1, nc, nd));
                        p += 1;
                        continue;
                    }
                    for q in p + 2..p + mc {
                        busca.inserir(q);
                    }
                } else {
                    for q in p + 1..p + mc {
                        busca.inserir(q);
                    }
                }
                e.casamento(&mut rc, p, mc, md);
                p += mc;
                continue;
            }
            let d0 = e.m.reps[0] as usize + 1;
            if d0 <= p && dados[p] == dados[p - d0] && e.m.estado >= 7 {
                e.rep_curto(&mut rc, p);
            } else {
                e.literal(&mut rc, p);
            }
            p += 1;
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
            // pedaco LZMA recomeca do estado inicial, dos dois lados.
            e.m.reiniciar(props);
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
