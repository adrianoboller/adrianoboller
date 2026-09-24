//! Codificador LZMA2 -- o que faz o PhxZip comprimir, e nao so empacotar.
//!
//! # O que ele faz
//!
//! Busca por dispersao de 4 bytes -- cadeia nos niveis 1 a 4, arvore
//! binaria nos 5 a 9, com cabecas proprias para os casamentos de 2 e 3
//! bytes -- e as quatro distancias repetidas conferidas a cada posicao. Do
//! nivel 5 em diante a escolha e por PRECO em bits, caminho minimo sobre os
//! simbolos possiveis (`otimo.rs`); do 1 ao 4, gulosa com um passo de
//! preguica.
//!
//! Medido contra o `7z -mf=off -mmt=1` (`docs/PHXZIP.md` §3), no mesmo
//! trabalho (mesmo dicionario, mesma profundidade, mesmo «bom»).
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

/// Quanto se procura: dicionario, profundidade da busca, «bom o bastante».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nivel {
    /// Tamanho do dicionario, em bytes (potencia de 2).
    pub dicionario: u32,
    /// Quantos candidatos (da cadeia ou da arvore) se conferem por posicao.
    pub profundidade: u32,
    /// Casamento deste tamanho encerra a busca.
    pub bom: usize,
    /// Analise preguicosa de um passo.
    pub preguicoso: bool,
    /// Analise otima por preco (o `GetOptimum` do 7-Zip, reescrito aqui).
    pub otimo: bool,
    /// Busca por arvore binaria em vez de cadeia (ver `Buscador`).
    pub arvore: bool,
    /// Casamentos de 2 e 3 bytes, por dispersao propria.
    pub curtos: bool,
}

impl Nivel {
    /// Nivel de 1 (rapido) a 9 (maximo), na escala do `7z -mx`. Fora da faixa
    /// satura.
    pub fn de(n: u8) -> Nivel {
        // Do 5 em diante a escolha e por preco em bits, e o «bom» e a
        // profundidade seguem o 7-Zip (fb e mc): o preco faz a busca render.
        // O 1 tambem segue (`7z -mx1`: 256 KiB, mc 16, fb 32): com 64 KiB e
        // profundidade 4 ele era mais rapido por fazer MENOS trabalho, e a
        // comparacao com o 7-Zip deixava de ser de trabalho igual.
        let (log_dic, profundidade, bom, preguicoso, otimo) = match n {
            0 | 1 => (18, 16, 32, false, false),
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
            arvore: otimo,
            curtos: otimo,
        }
    }
}

/// Posicao guardada como `p + 1`: o zero e «vazio».
const VAZIO: u32 = 0;
/// Teto das cabecas curtas (2 e 3 bytes): 64 Ki cada, 256 KiB. Com 16 bits
/// a de 2 bytes e indice exato; abaixo disso as duas sao dispersao, e o
/// candidato se confere byte a byte.
const BITS_CURTOS_MAX: u32 = 16;

/// A busca de casamentos. Duas estruturas sobre a mesma dispersao de 4 bytes:
///
/// - **cadeia** (niveis 1 a 4): cada posicao aponta a anterior com a mesma
///   dispersao; barata de inserir, mas confere candidatos em ordem de idade,
///   e o longo que mora fundo na cadeia fica fora da profundidade.
/// - **arvore binaria** (5 a 9): cada dispersao e a raiz de uma arvore de
///   posicoes ordenadas pelo sufixo. Descer dela confere os candidatos em
///   ordem de PREFIXO COMUM crescente, entao o casamento longo aparece com
///   poucos passos mesmo no texto repetitivo -- que era onde a cadeia perdia.
///   Custa o dobro por byte de janela (dois filhos em vez de um anterior), e
///   a insercao ja e a propria busca: nao existe inserir sem descer.
///
/// A ideia e a do `bt4` do `LzFind.c` do 7-Zip (dominio publico), reescrita:
/// aqui a insercao e monotona e a busca pede a posicao, em vez de o
/// localizador empurrar a posicao seguinte.
pub(super) struct Buscador<'a> {
    dados: &'a [u8],
    cabeca: Vec<u32>,
    /// Cadeia: um anterior por posicao da janela. Arvore: dois filhos
    /// (menor, maior) por posicao do ciclo.
    filhos: Vec<u32>,
    cab2: Vec<u32>,
    cab3: Vec<u32>,
    mascara: usize,
    bits: u32,
    bits_curtos: u32,
    janela: usize,
    /// Tamanho do ciclo da arvore: `janela + 1`, para a distancia igual ao
    /// dicionario (a maior que o decodificador aceita) nao cair no slot da
    /// propria posicao que esta entrando.
    ciclo: usize,
    profundidade: u32,
    bom: usize,
    arvore: bool,
    curtos: bool,
    /// Proxima posicao a entrar. A insercao e monotona: cada posicao entra
    /// uma vez, na ordem -- quem pula um trecho (um casamento) nao precisa
    /// lembrar de inserir o que pulou, e quem volta atras (o planejador que
    /// descartou um plano) nao insere duas vezes.
    proximo: usize,
}

impl<'a> Buscador<'a> {
    fn novo(dados: &'a [u8], janela: usize, nivel: &Nivel) -> Buscador<'a> {
        // As tabelas acompanham a janela, e a janela acompanha o dado: o
        // mesmo nivel 5 que gasta 50 MiB num arquivo de 4 MiB tem de caber
        // num microcontrolador comprimindo 4 KiB. Na arvore a colisao de
        // dispersao pesa mais que na cadeia (cada raiz vira uma arvore mais
        // funda), por isso ela ganha meia cabeca por byte de janela.
        let log = janela.trailing_zeros();
        let bits = if nivel.arvore {
            (log - 1).clamp(12, 22)
        } else if dados.len() < (1 << 16) {
            14
        } else {
            18
        };
        let bits_curtos = log.clamp(10, BITS_CURTOS_MAX);
        let ciclo = janela + 1;
        let filhos = if nivel.arvore {
            vec![VAZIO; 2 * ciclo]
        } else {
            vec![VAZIO; janela]
        };
        let (cab2, cab3) = if nivel.curtos {
            (vec![VAZIO; 1 << bits_curtos], vec![VAZIO; 1 << bits_curtos])
        } else {
            (Vec::new(), Vec::new())
        };
        Buscador {
            dados,
            cabeca: vec![VAZIO; 1 << bits],
            filhos,
            cab2,
            cab3,
            mascara: janela - 1,
            bits,
            bits_curtos,
            janela,
            ciclo,
            profundidade: nivel.profundidade,
            bom: nivel.bom.min(COMPR_MAX),
            arvore: nivel.arvore,
            curtos: nivel.curtos,
            proximo: 0,
        }
    }

    #[inline]
    fn dispersao(&self, p: usize) -> usize {
        let d = self.dados;
        let v = u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - self.bits)) as usize
    }

    #[inline]
    fn dispersao2(&self, p: usize) -> usize {
        let d = self.dados;
        let v = u16::from_le_bytes([d[p], d[p + 1]]) as u32;
        if self.bits_curtos == 16 {
            v as usize
        } else {
            (v.wrapping_mul(0x9E37_79B1) >> (32 - self.bits_curtos)) as usize
        }
    }

    #[inline]
    fn dispersao3(&self, p: usize) -> usize {
        let d = self.dados;
        let v = u32::from_le_bytes([d[p], d[p + 1], d[p + 2], 0]);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - self.bits_curtos)) as usize
    }

    /// Candidatos curtos em `p` (o anterior de mesma dispersao de 2 e de 3
    /// bytes), ja trocando as cabecas por `p`. Devolve as distancias, zero
    /// quando nao ha.
    #[inline]
    fn curtos_em(&mut self, p: usize) -> (usize, usize) {
        let h2 = self.dispersao2(p);
        let h3 = self.dispersao3(p);
        let c2 = core::mem::replace(&mut self.cab2[h2], p as u32 + 1);
        let c3 = core::mem::replace(&mut self.cab3[h3], p as u32 + 1);
        let dist = |c: u32| {
            if c == VAZIO {
                0
            } else {
                let dd = p - (c as usize - 1);
                if dd > self.janela {
                    0
                } else {
                    dd
                }
            }
        };
        (dist(c2), dist(c3))
    }

    fn inserir_proximo(&mut self) {
        let p = self.proximo;
        self.proximo += 1;
        if p + 4 > self.dados.len() {
            return;
        }
        if self.curtos {
            self.curtos_em(p);
        }
        let h = self.dispersao(p);
        let cand = core::mem::replace(&mut self.cabeca[h], p as u32 + 1);
        if self.arvore {
            let lim = self.bom.min(self.dados.len() - p);
            self.descer(p, cand, lim, usize::MAX, None);
        } else {
            self.filhos[p & self.mascara] = cand;
        }
    }

    /// Desce a arvore da raiz `cand` pondo `p` no lugar dela, e colhe em
    /// `saida` (quando ha) os casamentos maiores que `melhor`.
    ///
    /// O caminho de `p` divide a arvore em dois: o que e menor que o sufixo
    /// de `p` vai pendurado a esquerda dele, o maior a direita. `menor` e
    /// `maior` sao os ponteiros ainda abertos de cada lado, e o prefixo que
    /// cada lado ja confirmou (`compr_menor`, `compr_maior`) e o ponto de
    /// partida da comparacao seguinte -- os dois limitam o sufixo por baixo e
    /// por cima, entao o candidato seguinte casa ao menos o minimo dos dois.
    ///
    /// Casamento de `lim` bytes toma o lugar do no casado: a arvore trata
    /// como iguais os sufixos que empatam ate o limite, e e isso que impede
    /// o dado todo repetido de virar uma lista.
    fn descer(
        &mut self,
        p: usize,
        mut cand: u32,
        lim: usize,
        mut melhor: usize,
        mut saida: Option<&mut Vec<(usize, usize)>>,
    ) {
        let d = self.dados;
        let ciclo = self.ciclo;
        let pos = p % ciclo;
        let mut maior = 2 * pos + 1;
        let mut menor = 2 * pos;
        let (mut compr_maior, mut compr_menor) = (0usize, 0usize);
        let mut resta = self.profundidade;
        loop {
            if cand == VAZIO {
                break;
            }
            let c = cand as usize - 1;
            let delta = p - c;
            if resta == 0 || delta >= ciclo {
                break;
            }
            resta -= 1;
            let par = 2 * if delta > pos {
                pos + ciclo - delta
            } else {
                pos - delta
            };
            let mut n = compr_maior.min(compr_menor);
            if d[c + n] == d[p + n] {
                n += 1;
                while n < lim && d[c + n] == d[p + n] {
                    n += 1;
                }
                if n > melhor {
                    melhor = n;
                    if let Some(s) = saida.as_deref_mut() {
                        s.push((n, delta));
                    }
                }
                if n == lim {
                    self.filhos[menor] = self.filhos[par];
                    self.filhos[maior] = self.filhos[par + 1];
                    return;
                }
            }
            if d[c + n] < d[p + n] {
                self.filhos[menor] = cand;
                menor = par + 1;
                cand = self.filhos[menor];
                compr_menor = n;
            } else {
                self.filhos[maior] = cand;
                maior = par;
                cand = self.filhos[maior];
                compr_maior = n;
            }
        }
        self.filhos[menor] = VAZIO;
        self.filhos[maior] = VAZIO;
    }

    /// Todos os casamentos em `p` que melhoram o comprimento, do mais curto
    /// ao mais longo: para cada comprimento, a entrada que o alcanca primeiro
    /// e a mais perto que a busca viu -- e o que o preco quer.
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
        let max = (d.len() - p).min(COMPR_MAX);
        let h = self.dispersao(p);
        if self.proximo > p {
            // Posicao ja inserida: um plano descartado no fim de um pedaco
            // cru. A arvore nao se consulta sem se reescrever, e sem
            // casamento aqui so custa um literal; a cadeia ainda anda,
            // pulando quem entrou depois de `p`.
            if !self.arvore {
                self.cadeia(p, self.cabeca[h], 0, max, saida);
            }
            return;
        }
        self.proximo += 1;
        let mut melhor = 0usize;
        if self.curtos {
            let (d2, d3) = self.curtos_em(p);
            let mut dist = 0;
            if d2 != 0 && d[p - d2..p - d2 + 2] == d[p..p + 2] {
                melhor = 2;
                dist = d2;
                saida.push((2, d2));
            }
            if d3 != 0 && d3 != d2 && d[p - d3..p - d3 + 3] == d[p..p + 3] {
                melhor = 3;
                dist = d3;
                saida.push((3, d3));
            }
            if melhor > 0 {
                melhor = compr_comum(d, p, dist, max);
                if let Some(ultimo) = saida.last_mut() {
                    ultimo.0 = melhor;
                }
            }
        }
        let cand = core::mem::replace(&mut self.cabeca[h], p as u32 + 1);
        if self.arvore {
            let lim = self.bom.min(max);
            // O curto ja do tamanho do limite: a arvore so insere.
            // E nada abaixo do minimo: um byte so, que a dispersao deixa
            // passar numa colisao, nao e casamento.
            let desde = if melhor >= lim {
                usize::MAX
            } else {
                melhor.max(COMPR_MIN - 1)
            };
            self.descer(p, cand, lim, desde, Some(saida));
            // A arvore compara ate `lim`; quem chegou nele pode ir alem.
            if let Some(ultimo) = saida.last_mut() {
                if ultimo.0 == lim && lim < max {
                    ultimo.0 = compr_comum(d, p, ultimo.1, max);
                }
            }
        } else {
            self.filhos[p & self.mascara] = cand;
            if melhor < self.bom && melhor < max {
                self.cadeia(p, cand, melhor, max, saida);
            }
        }
    }

    fn cadeia(
        &self,
        p: usize,
        mut cand: u32,
        mut melhor: usize,
        max: usize,
        saida: &mut Vec<(usize, usize)>,
    ) {
        let d = self.dados;
        let mut resta = self.profundidade;
        while cand != VAZIO && resta > 0 {
            let c = cand as usize - 1;
            let prox = self.filhos[c & self.mascara];
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

    /// Texto de tabela: linhas quase iguais, o caso em que a busca e o
    /// preco mais se provam.
    fn tabela(linhas: u32) -> Vec<u8> {
        let mut t = Vec::new();
        for i in 0..linhas {
            let linha = alloc::format!(
                "| {} | pedido {} do dono | estado {} | medido em {} |\n",
                i % 97,
                i * 7 % 450,
                ["aberto", "feito", "parcial"][(i % 3) as usize],
                i % 31
            );
            t.extend_from_slice(linha.as_bytes());
        }
        t
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
        let t = tabela(4000);
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

    /// Confere cada casamento que a busca devolve em cada posicao: os bytes
    /// batem, a distancia cabe na janela, e os comprimentos crescem. E o que
    /// a arvore pode quebrar calada -- um ponteiro errado de filho ainda
    /// comprime e ainda volta, so que um casamento falso sai como dado errado.
    fn conferir_busca(d: &[u8], janela: usize, nivel: &Nivel) -> usize {
        let mut b = Buscador::novo(d, janela, nivel);
        let mut lista = Vec::new();
        let mut maior = 0;
        let mut p = 0;
        while p < d.len() {
            b.todos(p, &mut lista);
            let mut ant = 0;
            for &(l, dist) in &lista {
                assert!(l > ant, "comprimentos fora de ordem em {p}: {lista:?}");
                // Casamento de um byte custou um preco de comprimento
                // indefinido no planejador (e estouro na conta, em debug).
                assert!(!nivel.arvore || l >= COMPR_MIN, "casamento de {l} em {p}");
                assert!(
                    dist >= 1 && dist <= janela && dist <= p,
                    "distancia {dist} em {p}"
                );
                assert!(p + l <= d.len());
                assert_eq!(compr_comum(d, p, dist, l), l, "casamento falso em {p}");
                ant = l;
            }
            maior = maior.max(ant);
            // Pula como o codificador pula: as posicoes do meio entram pela
            // insercao monotona, sem consulta.
            p += if ant >= 8 { ant } else { 1 };
        }
        maior
    }

    #[test]
    fn a_busca_so_devolve_casamento_verdadeiro_em_dado_adversario() {
        let arvore = Nivel::de(5);
        let cadeia = Nivel::de(1);
        let casos: [Vec<u8>; 5] = [
            vec![0u8; 20_000],
            tabela(1500),
            b"ab".repeat(9_000),
            pseudo(30_000, 5),
            {
                let mut v = pseudo(3_000, 8);
                for k in 0..6_000u32 {
                    v.push((k % 7) as u8);
                    v.push((k / 7 % 3) as u8);
                }
                v
            },
        ];
        for d in &casos {
            for janela in [4096, 1 << 16] {
                conferir_busca(d, janela, &arvore);
                conferir_busca(d, janela, &cadeia);
            }
        }
        // O tudo-zero casa o maximo em quase toda posicao.
        assert_eq!(conferir_busca(&casos[0], 4096, &arvore), COMPR_MAX);
        assert!(conferir_busca(&casos[1], 1 << 16, &arvore) >= 40);
    }

    /// O casamento longo que mora FUNDO: mais de `profundidade` vizinhos
    /// recentes dividem com ele os primeiros oito bytes. A cadeia confere
    /// em ordem de idade e para antes de chegar nele; a arvore desce por
    /// prefixo comum e o acha. Se os niveis 5-9 voltarem a cadeia, cai.
    #[test]
    fn a_arvore_acha_o_casamento_longo_que_a_cadeia_perde() {
        let bloco = pseudo(400, 21);
        let mut d = bloco.clone();
        for k in 0..200u32 {
            d.extend_from_slice(&bloco[..8]);
            d.extend_from_slice(&pseudo(24, 1000 + k));
        }
        let alvo = d.len();
        d.extend_from_slice(&bloco);
        let achado = |nivel: Nivel| {
            let mut b = Buscador::novo(&d, 1 << 16, &nivel);
            let mut lista = Vec::new();
            b.todos(alvo, &mut lista);
            lista.last().copied().unwrap_or((0, 0))
        };
        assert_eq!(achado(Nivel::de(5)), (COMPR_MAX, alvo));
        assert_eq!(achado(Nivel::de(9)), (COMPR_MAX, alvo));
        // A mesma busca pela cadeia fica no prefixo -- o que prova que o
        // dado de fato separa as duas.
        let (l, _) = achado(Nivel {
            arvore: false,
            ..Nivel::de(5)
        });
        assert!(l < 16, "a cadeia achou {l}");
        ida_e_volta(&d, 5);
    }

    /// Casamento de 2 e de 3 bytes: a dispersao de 4 nunca os ve. Se os
    /// niveis 5-9 perderem as cabecas curtas, cai.
    #[test]
    fn casamentos_de_dois_e_tres_bytes() {
        let d = b"xyQ...........xyzR..........xyz_";
        let mut b = Buscador::novo(d, 4096, &Nivel::de(5));
        let mut lista = Vec::new();
        let p2 = 14; // "xyzR": so "xy" ja apareceu
        assert_eq!(&d[p2..p2 + 4], b"xyzR");
        b.todos(p2, &mut lista);
        assert_eq!(lista.last(), Some(&(2, 14)), "{lista:?}");
        let p3 = 28; // "xyz_": "xyz" ja apareceu
        assert_eq!(&d[p3..p3 + 4], b"xyz_");
        b.todos(p3, &mut lista);
        assert_eq!(lista.last(), Some(&(3, 14)), "{lista:?}");
    }

    /// A distancia igual ao dicionario e a maior que o decodificador
    /// aceita, e mora no slot vizinho do ciclo da arvore: um byte a menos
    /// no ciclo e ela cai no slot da propria posicao que entra. Aleatorio
    /// repetido exatamente a `janela` so comprime por ela; a um byte alem,
    /// nao comprime.
    #[test]
    fn distancia_no_limite_da_janela() {
        let r = pseudo(4096, 33);
        // Sem as cabecas curtas, que achariam a mesma distancia por outro
        // caminho e esconderiam o limite da arvore.
        let nivel = Nivel {
            dicionario: 4096,
            curtos: false,
            ..Nivel::de(9)
        };
        let mut d = r.clone();
        d.extend_from_slice(&r);
        let mut b = Buscador::novo(&d, 4096, &nivel);
        let mut lista = Vec::new();
        b.todos(4096, &mut lista);
        assert_eq!(lista.last(), Some(&(COMPR_MAX, 4096)));
        let (p, c) = codificar_lzma2(&d, nivel);
        assert!(c.len() < 4096 + 400, "{}", c.len());
        let mut v = Vec::new();
        decodificar_lzma2(p, &c, d.len(), &mut v).unwrap();
        assert_eq!(v, d);

        let mut longe = r.clone();
        longe.push(7);
        longe.extend_from_slice(&r);
        let mut b = Buscador::novo(&longe, 4096, &nivel);
        b.todos(4097, &mut lista);
        assert!(lista.iter().all(|&(_, dist)| dist <= 4096), "{lista:?}");
        let (p, c) = codificar_lzma2(&longe, nivel);
        let mut v = Vec::new();
        decodificar_lzma2(p, &c, longe.len(), &mut v).unwrap();
        assert_eq!(v, longe);
    }

    #[test]
    fn ida_e_volta_adversaria_em_todos_os_niveis() {
        let mut periodico = Vec::new();
        for periodo in 1..=9usize {
            let base = pseudo(periodo, periodo as u32);
            for _ in 0..(3000 / periodo) {
                periodico.extend_from_slice(&base);
            }
        }
        let casos = [
            vec![0u8; 1 << 20],
            periodico,
            pseudo(200_000, 44),
            b"O pai veio antes do filho. ".repeat(20_000),
        ];
        for d in &casos {
            for nivel in 1..=9 {
                ida_e_volta(d, nivel);
            }
        }
    }
}
