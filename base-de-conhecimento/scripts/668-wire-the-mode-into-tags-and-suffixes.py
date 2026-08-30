# Wire the mode into tags and suffixes
# 28/08 18:45

import io
p='crates/phxsql-core/src/paginacao.rs'
s=io.open(p,encoding='utf-8').read()

velho='''impl ModoParticao {
    pub fn periodo(&self) -> Option<Periodo> {
        match self {
            ModoParticao::PorQuantidade => None,
            ModoParticao::PorPeriodo { periodo, .. } => Some(*periodo),
        }
    }

    pub fn coluna(&self) -> Option<usize> {
        match self {
            ModoParticao::PorQuantidade => None,
            ModoParticao::PorPeriodo { coluna, .. } => Some(*coluna as usize),
        }
    }

    /// Dois bytes na serializacao: tag do periodo (0 = por quantidade) e, se
    /// houver, a coluna.
    pub fn tag(&self) -> (u8, u16) {
        match self {
            ModoParticao::PorQuantidade => (0, 0),
            ModoParticao::PorPeriodo { coluna, periodo } => (periodo.tag(), *coluna),
        }
    }

    pub fn de_tag(tag: u8, coluna: u16) -> Result<ModoParticao> {
        Ok(match tag {
            0 => ModoParticao::PorQuantidade,
            outro => ModoParticao::PorPeriodo {
                coluna,
                periodo: Periodo::de_tag(outro)?,
            },
        })
    }
}'''

novo='''/// Tag da particao alfanumerica na serializacao do esquema.
///
/// Escolhida bem longe das do periodo (1..=4) de proposito: um byte trocado
/// entre elas trocaria o modo da tabela, e o modo decide o ENDERECO de cada
/// linha. Longe, o byte torto cai em "tag desconhecida" e o esquema e recusado.
const TAG_POR_LETRA: u8 = 200;

impl ModoParticao {
    pub fn periodo(&self) -> Option<Periodo> {
        match self {
            ModoParticao::PorPeriodo { periodo, .. } => Some(*periodo),
            _ => None,
        }
    }

    pub fn coluna(&self) -> Option<usize> {
        match self {
            ModoParticao::PorQuantidade => None,
            ModoParticao::PorPeriodo { coluna, .. } | ModoParticao::PorLetra { coluna } => {
                Some(*coluna as usize)
            }
        }
    }

    /// A particao e alfanumerica?
    pub fn por_letra(&self) -> bool {
        matches!(self, ModoParticao::PorLetra { .. })
    }

    /// O nome deste modo, como a tela e o `.pag` escrevem.
    pub fn nome(&self) -> &'static str {
        match self {
            ModoParticao::PorQuantidade => "quantidade",
            ModoParticao::PorPeriodo { .. } => "periodo",
            ModoParticao::PorLetra { .. } => "letra",
        }
    }

    /// Dois bytes na serializacao: tag do modo (0 = por quantidade) e, se
    /// houver, a coluna.
    pub fn tag(&self) -> (u8, u16) {
        match self {
            ModoParticao::PorQuantidade => (0, 0),
            ModoParticao::PorPeriodo { coluna, periodo } => (periodo.tag(), *coluna),
            ModoParticao::PorLetra { coluna } => (TAG_POR_LETRA, *coluna),
        }
    }

    pub fn de_tag(tag: u8, coluna: u16) -> Result<ModoParticao> {
        Ok(match tag {
            0 => ModoParticao::PorQuantidade,
            TAG_POR_LETRA => ModoParticao::PorLetra { coluna },
            outro => ModoParticao::PorPeriodo {
                coluna,
                periodo: Periodo::de_tag(outro)?,
            },
        })
    }
}'''
assert velho in s
s=s.replace(velho,novo,1)

# sufixo com letra
velho2='''    /// Sufixo do nome do arquivo: `_001` com paginacao, vazio sem.
    pub fn sufixo(&self, volume: u32) -> String {
        if !self.ligada() {
            String::new()
        } else {
            format!("_{:0largura$}", volume, largura = self.digitos as usize)
        }'''
novo2='''    /// Sufixo do nome do arquivo: `_001` com paginacao, `_A` na alfanumerica,
    /// vazio sem paginacao.
    pub fn sufixo(&self, volume: u32) -> String {
        if !self.ligada() {
            String::new()
        } else if self.modo.por_letra() {
            // Fora da faixa nao acontece por construcao, mas um nome de
            // arquivo e o ultimo lugar onde se quer um `unwrap`: um volume
            // desconhecido vira `_Outros`, que existe e e legivel.
            let i = (volume as usize).clamp(1, BALDES.len());
            format!("_{}", BALDES[i - 1])
        } else {
            format!("_{:0largura$}", volume, largura = self.digitos as usize)
        }'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
