//! Analise otima por preco: escolher o CAMINHO de simbolos mais barato em
//! bits, e nao o maior casamento de cada posicao.
//!
//! # A ideia
//!
//! O guloso pega o maior casamento que ve. Mas um casamento de 5 bytes a
//! distancia 40.000 pode custar mais que um literal seguido de uma repeticao
//! de 6 a distancia que ja esta guardada. O 7-Zip (`GetOptimum` em
//! `LzmaEnc.c`, dominio publico) resolve isso como caminho minimo num grafo:
//! cada posicao a frente e um no, cada simbolo possivel e uma aresta com o
//! preco em bits dela, e o preco sai das probabilidades ATUAIS do modelo.
//!
//! # Onde este diverge do de la, e por que
//!
//! - **O estado de cada no se calcula quando o no e visitado**, a partir do
//!   no anterior e da aresta que chegou nele, e nao ao relaxar a aresta. O de
//!   la carrega varios casos especiais (literal seguido de repeticao na mesma
//!   aresta, `Prev2`) para ganhar alguns bytes; aqui a aresta e sempre UM
//!   simbolo, e a corretude do plano se confere lendo `derivar` inteiro.
//! - **Os precos de comprimento e distancia se refazem por volume** (a cada
//!   `REFAZER` bytes), e nao por contador de uso. O desvio entre os dois e
//!   pequeno, e o nosso nao pede contador em cada simbolo emitido.
//! - **Os casamentos saem da cadeia de dispersao de 3 bytes**, sem arvore
//!   binaria (`bt4`). Comprimento 2 so por repeticao.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use super::enc::{compr_comum, Buscador, Decisao};
use super::modelo::{
    estado_de_compr, Comprimentos, Modelo, COMPR_MAX, COMPR_MIN, DISTANCIAS_CHEIAS,
    ESTADOS_DE_COMPR, FIM_DO_MODELO_DE_POS, POS_MAX,
};

/// Nos a frente que um plano pode olhar.
const JANELA: usize = 1 << 11;
/// Bytes emitidos entre duas atualizacoes das tabelas de preco.
const REFAZER: usize = 1 << 11;
const INFINITO: u32 = u32::MAX;

/// Preco de cada probabilidade, em 1/16 de bit: -log2(p/2048) * 16, pelo
/// mesmo laco inteiro do 7-Zip (quatro quadraturas) -- sem `log2`, que o
/// `core` nao tem.
const fn gerar_precos() -> [u32; 128] {
    let mut t = [0u32; 128];
    let mut i = 0;
    while i < 128 {
        let mut w: u32 = ((i as u32) << 4) + 8;
        let mut contagem: u32 = 0;
        let mut j = 0;
        while j < 4 {
            w = w * w;
            contagem <<= 1;
            while w >= 1 << 16 {
                w >>= 1;
                contagem += 1;
            }
            j += 1;
        }
        t[i] = (11 << 4) - 15 - contagem;
        i += 1;
    }
    t
}

static PRECOS: [u32; 128] = gerar_precos();

#[inline]
fn p_bit(prob: u16, b: u32) -> u32 {
    PRECOS[(((prob as u32) ^ (0u32.wrapping_sub(b) & 0x7FF)) >> 4) as usize]
}

fn p_arvore(probs: &[u16], bits: u32, v: u32) -> u32 {
    let mut m = 1usize;
    let mut s = 0;
    for i in (0..bits).rev() {
        let b = (v >> i) & 1;
        s += p_bit(probs[m], b);
        m = (m << 1) | b as usize;
    }
    s
}

fn p_arvore_reversa(probs: &[u16], bits: u32, mut v: u32) -> u32 {
    let mut m = 1usize;
    let mut s = 0;
    for _ in 0..bits {
        let b = v & 1;
        v >>= 1;
        s += p_bit(probs[m], b);
        m = (m << 1) | b as usize;
    }
    s
}

fn p_compr(c: &Comprimentos, pe: usize, compr: usize) -> u32 {
    let l = (compr - COMPR_MIN) as u32;
    if l < 8 {
        p_bit(c.escolha, 0) + p_arvore(&c.baixo[pe], 3, l)
    } else if l < 16 {
        p_bit(c.escolha, 1) + p_bit(c.escolha2, 0) + p_arvore(&c.meio[pe], 3, l - 8)
    } else {
        p_bit(c.escolha, 1) + p_bit(c.escolha2, 1) + p_arvore(&c.alto, 8, l - 16)
    }
}

fn fenda_de(v: u32) -> u32 {
    if v < 4 {
        v
    } else {
        let n = 31 - v.leading_zeros();
        (n << 1) | ((v >> (n - 1)) & 1)
    }
}

/// As tabelas de preco refeitas a partir do modelo.
struct Precos {
    compr: Vec<[u32; COMPR_MAX + 1]>,
    compr_rep: Vec<[u32; COMPR_MAX + 1]>,
    fenda: [[u32; 64]; ESTADOS_DE_COMPR],
    dist: [[u32; DISTANCIAS_CHEIAS]; ESTADOS_DE_COMPR],
    alinhamento: [u32; 16],
}

impl Precos {
    fn novo() -> Precos {
        Precos {
            compr: vec![[0; COMPR_MAX + 1]; POS_MAX],
            compr_rep: vec![[0; COMPR_MAX + 1]; POS_MAX],
            fenda: [[0; 64]; ESTADOS_DE_COMPR],
            dist: [[0; DISTANCIAS_CHEIAS]; ESTADOS_DE_COMPR],
            alinhamento: [0; 16],
        }
    }

    fn refazer(&mut self, m: &Modelo) {
        let estados_pos = 1usize << m.props.pb;
        for pe in 0..estados_pos {
            for l in COMPR_MIN..=COMPR_MAX {
                self.compr[pe][l] = p_compr(&m.compr, pe, l);
                self.compr_rep[pe][l] = p_compr(&m.compr_rep, pe, l);
            }
        }
        for ls in 0..ESTADOS_DE_COMPR {
            for f in 0..64u32 {
                let mut p = p_arvore(&m.fenda[ls], 6, f);
                if f >= FIM_DO_MODELO_DE_POS {
                    // Bits diretos custam um bit cheio cada: 16 em 1/16.
                    p += ((f >> 1) - 1 - 4) << 4;
                }
                self.fenda[ls][f as usize] = p;
            }
            for v in 0..DISTANCIAS_CHEIAS as u32 {
                let f = fenda_de(v);
                let mut p = self.fenda[ls][f as usize];
                if f >= 4 {
                    let n = (f >> 1) - 1;
                    let base = (2 | (f & 1)) << n;
                    p += p_arvore_reversa(&m.especial[(base - f) as usize..], n, v - base);
                }
                self.dist[ls][v as usize] = p;
            }
        }
        for (i, a) in self.alinhamento.iter_mut().enumerate() {
            *a = p_arvore_reversa(&m.alinhamento, 4, i as u32);
        }
    }

    /// Preco da distancia `v` (= distancia - 1) para um casamento de `compr`.
    fn distancia(&self, v: u32, compr: usize) -> u32 {
        let ls = estado_de_compr(compr);
        if (v as usize) < DISTANCIAS_CHEIAS {
            self.dist[ls][v as usize]
        } else {
            self.fenda[ls][fenda_de(v) as usize] + self.alinhamento[(v & 15) as usize]
        }
    }
}

#[derive(Clone, Copy)]
struct No {
    preco: u32,
    ant: u32,
    dec: Decisao,
    estado: u8,
    reps: [u32; 4],
}

/// O planejador. Guarda os nos e as tabelas entre uma chamada e outra.
pub(super) struct Otimo {
    nos: Vec<No>,
    lista: Vec<(usize, usize)>,
    precos: Precos,
    desde: usize,
    pronto: bool,
}

fn apos_literal(e: u8) -> u8 {
    if e < 4 {
        0
    } else if e < 10 {
        e - 3
    } else {
        e - 6
    }
}

impl Otimo {
    pub(super) fn novo() -> Otimo {
        let vazio = No {
            preco: INFINITO,
            ant: 0,
            dec: Decisao::Literal,
            estado: 0,
            reps: [0; 4],
        };
        Otimo {
            nos: vec![vazio; JANELA + COMPR_MAX + 2],
            lista: Vec::new(),
            precos: Precos::novo(),
            desde: 0,
            pronto: false,
        }
    }

    fn p_literal(m: &Modelo, d: &[u8], pos: usize, estado: u8, rep0: u32) -> u32 {
        let anterior = if pos > 0 { d[pos - 1] } else { 0 };
        let base = m.base_literal(pos as u64, anterior);
        let probs = &m.literais[base..base + 0x300];
        let byte = d[pos] as usize;
        let mut s = 1usize;
        let mut preco = 0;
        let mut casado = estado >= 7;
        let alvo = if casado {
            d[pos - rep0 as usize - 1] as usize
        } else {
            0
        };
        for i in (0..8).rev() {
            let b = (byte >> i) & 1;
            if casado {
                let bc = (alvo >> i) & 1;
                preco += p_bit(probs[0x100 + (bc << 8) + s], b as u32);
                casado = bc == b;
            } else {
                preco += p_bit(probs[s], b as u32);
            }
            s = (s << 1) | b;
        }
        preco
    }

    fn p_indice_rep(m: &Modelo, i: usize, st: usize, pe: usize) -> u32 {
        match i {
            0 => p_bit(m.e_rep_g0[st], 0) + p_bit(m.e_rep0_longo[st][pe], 1),
            1 => p_bit(m.e_rep_g0[st], 1) + p_bit(m.e_rep_g1[st], 0),
            _ => {
                p_bit(m.e_rep_g0[st], 1)
                    + p_bit(m.e_rep_g1[st], 1)
                    + p_bit(m.e_rep_g2[st], (i - 2) as u32)
            }
        }
    }

    #[inline]
    fn relaxar(&mut self, alvo: usize, preco: u32, ant: usize, dec: Decisao) {
        if preco < self.nos[alvo].preco {
            let n = &mut self.nos[alvo];
            n.preco = preco;
            n.ant = ant as u32;
            n.dec = dec;
        }
    }

    /// Calcula estado e distancias do no `cur` pelo no anterior e pela
    /// aresta que chegou nele -- a mesma transicao que `Escrita` faz ao
    /// emitir, e por isso o plano nunca diverge do que o decodificador ve.
    fn derivar(&mut self, cur: usize) {
        let n = self.nos[cur];
        let a = self.nos[n.ant as usize];
        let (estado, reps) = match n.dec {
            Decisao::Literal => (apos_literal(a.estado), a.reps),
            Decisao::RepCurto => (if a.estado < 7 { 9 } else { 11 }, a.reps),
            Decisao::Rep(i, _) => {
                let mut r = a.reps;
                let d = r[i as usize];
                for k in (1..=i as usize).rev() {
                    r[k] = r[k - 1];
                }
                r[0] = d;
                (if a.estado < 7 { 8 } else { 11 }, r)
            }
            Decisao::Casa(dist, _) => (
                if a.estado < 7 { 7 } else { 10 },
                [dist - 1, a.reps[0], a.reps[1], a.reps[2]],
            ),
        };
        self.nos[cur].estado = estado;
        self.nos[cur].reps = reps;
    }

    /// Relaxa todas as arestas que saem do no `cur` (posicao `p + cur`),
    /// com os casamentos de `self.lista`. Devolve o no mais longe alcancado.
    fn estender(&mut self, m: &Modelo, d: &[u8], p: usize, cur: usize) -> usize {
        let no = self.nos[cur];
        let pos = p + cur;
        let pe = m.pos_estado(pos as u64);
        let st = no.estado as usize;
        let max = (d.len() - pos).min(COMPR_MAX);
        let mut longe = cur + 1;

        let lit = no.preco
            + p_bit(m.e_casamento[st][pe], 0)
            + Self::p_literal(m, d, pos, no.estado, no.reps[0]);
        self.relaxar(cur + 1, lit, cur, Decisao::Literal);

        let bit1 = no.preco + p_bit(m.e_casamento[st][pe], 1);
        let base_rep = bit1 + p_bit(m.e_rep[st], 1);
        let d0 = no.reps[0] as usize + 1;
        if d0 <= pos && d[pos] == d[pos - d0] {
            let curto = base_rep + p_bit(m.e_rep_g0[st], 0) + p_bit(m.e_rep0_longo[st][pe], 0);
            self.relaxar(cur + 1, curto, cur, Decisao::RepCurto);
        }
        for i in 0..4 {
            let dist = no.reps[i] as usize + 1;
            if dist > pos {
                continue;
            }
            let l_max = compr_comum(d, pos, dist, max);
            if l_max < COMPR_MIN {
                continue;
            }
            let base = base_rep + Self::p_indice_rep(m, i, st, pe);
            for l in COMPR_MIN..=l_max {
                let preco = base + self.precos.compr_rep[pe][l];
                self.relaxar(cur + l, preco, cur, Decisao::Rep(i as u8, l as u16));
            }
            longe = longe.max(cur + l_max);
        }

        let base_casa = bit1 + p_bit(m.e_rep[st], 0);
        let mut l = COMPR_MIN;
        for k in 0..self.lista.len() {
            let (ml, md) = self.lista[k];
            while l <= ml {
                let preco = base_casa
                    + self.precos.compr[pe][l]
                    + self.precos.distancia((md - 1) as u32, l);
                self.relaxar(cur + l, preco, cur, Decisao::Casa(md as u32, l as u16));
                l += 1;
            }
            longe = longe.max(cur + ml);
        }
        longe
    }

    /// Planeja os simbolos a partir de `p` e os poe em `fila`. O modelo `m`
    /// tem de estar exatamente no estado de `p` (a fila vazia garante).
    pub(super) fn planejar(
        &mut self,
        m: &Modelo,
        d: &[u8],
        busca: &mut Buscador,
        p: usize,
        bom: usize,
        fila: &mut VecDeque<Decisao>,
    ) {
        if !self.pronto || self.desde >= REFAZER {
            self.precos.refazer(m);
            self.pronto = true;
            self.desde = 0;
        }
        let max = (d.len() - p).min(COMPR_MAX);
        busca.todos(p, &mut self.lista);

        // Atalhos: repeticao ou casamento «bom o bastante» sai direto.
        let mut rep_melhor = (0usize, 0usize);
        for (i, r) in m.reps.iter().enumerate() {
            let dist = *r as usize + 1;
            if dist <= p {
                let l = compr_comum(d, p, dist, max);
                if l > rep_melhor.0 {
                    rep_melhor = (l, i);
                }
            }
        }
        if rep_melhor.0 >= bom {
            self.emitir_direto(Decisao::Rep(rep_melhor.1 as u8, rep_melhor.0 as u16), fila);
            return;
        }
        let principal = self.lista.last().copied().unwrap_or((0, 0));
        if principal.0 >= bom {
            self.emitir_direto(Decisao::Casa(principal.1 as u32, principal.0 as u16), fila);
            return;
        }

        self.nos[0] = No {
            preco: 0,
            ant: 0,
            dec: Decisao::Literal,
            estado: m.estado as u8,
            reps: m.reps,
        };
        let mut fim = self.estender(m, d, p, 0);
        let mut tocado = fim.max(COMPR_MAX);
        let mut cur = 1;
        while cur < fim {
            self.derivar(cur);
            if cur >= JANELA {
                break;
            }
            busca.todos(p + cur, &mut self.lista);
            if let Some(&(l, dist)) = self.lista.last() {
                if l >= bom {
                    // Casamento longo a frente: o caminho passa por ele, sem
                    // olhar alem -- e o que corta o custo em dado repetitivo.
                    let n = &mut self.nos[cur + l];
                    n.preco = 0;
                    n.ant = cur as u32;
                    n.dec = Decisao::Casa(dist as u32, l as u16);
                    fim = cur + l;
                    tocado = tocado.max(fim);
                    break;
                }
            }
            let longe = self.estender(m, d, p, cur);
            fim = fim.max(longe);
            tocado = tocado.max(fim);
            cur += 1;
        }

        // Volta do fim ao comeco pelos ponteiros, e a fila sai na ordem.
        let inicio_fila = fila.len();
        let mut k = fim;
        while k > 0 {
            let n = self.nos[k];
            fila.push_back(n.dec);
            k = n.ant as usize;
        }
        fila.make_contiguous()[inicio_fila..].reverse();
        let andou: usize = fila.iter().skip(inicio_fila).map(|x| x.compr()).sum();
        self.desde += andou;

        let ate = tocado.min(self.nos.len() - 1);
        for n in self.nos[..=ate].iter_mut() {
            n.preco = INFINITO;
        }
    }

    fn emitir_direto(&mut self, dec: Decisao, fila: &mut VecDeque<Decisao>) {
        self.desde += dec.compr();
        fila.push_back(dec);
    }
}
