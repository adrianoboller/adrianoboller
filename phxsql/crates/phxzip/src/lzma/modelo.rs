//! O modelo de probabilidades do LZMA, comum ao codificador e ao decodificador.

use alloc::vec;
use alloc::vec::Vec;

use super::Props;

pub(crate) const ESTADOS: usize = 12;
pub(crate) const POS_MAX: usize = 1 << 4;
pub(crate) const PROB_INICIAL: u16 = 1024;
pub(crate) const COMPR_MIN: usize = 2;
pub(crate) const COMPR_MAX: usize = 273;
pub(crate) const FIM_DO_MODELO_DE_POS: u32 = 14;
pub(crate) const DISTANCIAS_CHEIAS: usize = 128;
pub(crate) const ESTADOS_DE_COMPR: usize = 4;

/// Codificador/decodificador de comprimento: escolha, escolha2, baixo, meio, alto.
#[derive(Clone)]
pub(crate) struct Comprimentos {
    pub escolha: u16,
    pub escolha2: u16,
    pub baixo: [[u16; 8]; POS_MAX],
    pub meio: [[u16; 8]; POS_MAX],
    pub alto: [u16; 256],
}

impl Comprimentos {
    fn novo() -> Comprimentos {
        Comprimentos {
            escolha: PROB_INICIAL,
            escolha2: PROB_INICIAL,
            baixo: [[PROB_INICIAL; 8]; POS_MAX],
            meio: [[PROB_INICIAL; 8]; POS_MAX],
            alto: [PROB_INICIAL; 256],
        }
    }
}

/// Todas as probabilidades e o estado da maquina.
#[derive(Clone)]
pub(crate) struct Modelo {
    pub props: Props,
    pub literais: Vec<u16>,
    pub e_casamento: [[u16; POS_MAX]; ESTADOS],
    pub e_rep: [u16; ESTADOS],
    pub e_rep_g0: [u16; ESTADOS],
    pub e_rep_g1: [u16; ESTADOS],
    pub e_rep_g2: [u16; ESTADOS],
    pub e_rep0_longo: [[u16; POS_MAX]; ESTADOS],
    pub fenda: [[u16; 64]; ESTADOS_DE_COMPR],
    pub especial: [u16; 1 + DISTANCIAS_CHEIAS - FIM_DO_MODELO_DE_POS as usize],
    pub alinhamento: [u16; 16],
    pub compr: Comprimentos,
    pub compr_rep: Comprimentos,
    pub estado: usize,
    pub reps: [u32; 4],
}

impl Modelo {
    pub fn novo(props: Props) -> Modelo {
        Modelo {
            props,
            literais: vec![PROB_INICIAL; 0x300 << (props.lc + props.lp)],
            e_casamento: [[PROB_INICIAL; POS_MAX]; ESTADOS],
            e_rep: [PROB_INICIAL; ESTADOS],
            e_rep_g0: [PROB_INICIAL; ESTADOS],
            e_rep_g1: [PROB_INICIAL; ESTADOS],
            e_rep_g2: [PROB_INICIAL; ESTADOS],
            e_rep0_longo: [[PROB_INICIAL; POS_MAX]; ESTADOS],
            fenda: [[PROB_INICIAL; 64]; ESTADOS_DE_COMPR],
            especial: [PROB_INICIAL; 1 + DISTANCIAS_CHEIAS - FIM_DO_MODELO_DE_POS as usize],
            alinhamento: [PROB_INICIAL; 16],
            compr: Comprimentos::novo(),
            compr_rep: Comprimentos::novo(),
            estado: 0,
            reps: [0; 4],
        }
    }

    /// Volta ao estado inicial, mantendo as propriedades (o «state reset» do LZMA2).
    pub fn reiniciar(&mut self, props: Props) {
        *self = Modelo::novo(props);
    }

    /// Indice da tabela de literais para a posicao e o byte anterior.
    pub fn base_literal(&self, pos: u64, anterior: u8) -> usize {
        let lp_mask = (1u64 << self.props.lp) - 1;
        let ctx = (((pos & lp_mask) as usize) << self.props.lc)
            + ((anterior as usize) >> (8 - self.props.lc));
        ctx * 0x300
    }

    pub fn pos_estado(&self, pos: u64) -> usize {
        (pos & ((1u64 << self.props.pb) - 1)) as usize
    }

    pub fn apos_literal(&mut self) {
        self.estado = if self.estado < 4 {
            0
        } else if self.estado < 10 {
            self.estado - 3
        } else {
            self.estado - 6
        };
    }

    pub fn apos_casamento(&mut self) {
        self.estado = if self.estado < 7 { 7 } else { 10 };
    }

    pub fn apos_rep(&mut self) {
        self.estado = if self.estado < 7 { 8 } else { 11 };
    }

    pub fn apos_rep_curto(&mut self) {
        self.estado = if self.estado < 7 { 9 } else { 11 };
    }
}

/// Estado de comprimento usado para escolher a tabela de fenda da distancia.
pub(crate) fn estado_de_compr(compr: usize) -> usize {
    (compr - COMPR_MIN).min(ESTADOS_DE_COMPR - 1)
}
