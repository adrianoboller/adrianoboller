//! Decodificador LZMA.
//!
//! Decodifica direto no `Vec` de saida, que faz as vezes de dicionario: o 7z
//! declara o tamanho descompactado de cada pasta, a saida inteira fica em
//! memoria de qualquer jeito (o chamador pediu os bytes), e um dicionario
//! circular separado seria uma segunda copia do mesmo dado. O teto de memoria
//! e o do tamanho declarado, conferido contra `Limites` antes de alocar.

use alloc::vec::Vec;

use super::faixa::Decodificador;
use super::modelo::{estado_de_compr, Comprimentos, Modelo, COMPR_MIN, FIM_DO_MODELO_DE_POS};
use super::Props;
use crate::erro::{Erro, Resultado};

/// Estado do decodificador que sobrevive entre os pedacos do LZMA2.
pub struct DecodificadorLzma {
    modelo: Modelo,
}

fn ler_compr(rc: &mut Decodificador, c: &mut Comprimentos, pe: usize) -> Resultado<usize> {
    let l = if rc.bit(&mut c.escolha)? == 0 {
        rc.arvore(&mut c.baixo[pe], 3)?
    } else if rc.bit(&mut c.escolha2)? == 0 {
        8 + rc.arvore(&mut c.meio[pe], 3)?
    } else {
        16 + rc.arvore(&mut c.alto, 8)?
    };
    Ok(l as usize + COMPR_MIN)
}

impl DecodificadorLzma {
    /// Novo, com as propriedades dadas.
    pub fn novo(props: Props) -> Resultado<DecodificadorLzma> {
        if props.lc > 8 || props.lp > 4 || props.pb > 4 {
            return Err(Erro::Corrompido("propriedades LZMA fora da faixa"));
        }
        Ok(DecodificadorLzma {
            modelo: Modelo::novo(props),
        })
    }

    /// Reinicia probabilidades e estado; troca as propriedades se vierem.
    pub(crate) fn reiniciar(&mut self, props: Option<Props>) {
        let p = props.unwrap_or(self.modelo.props);
        self.modelo.reiniciar(p);
    }

    /// Decodifica exatamente `quantos` bytes do fluxo `rc`, acrescentando em
    /// `saida`. `inicio` e onde o dicionario comeca em `saida` (o ultimo
    /// «dict reset» do LZMA2; zero no LZMA puro): distancia que aponte para
    /// antes dele e dado corrompido, e nunca leitura de dado alheio.
    pub(crate) fn decodificar(
        &mut self,
        rc: &mut Decodificador,
        saida: &mut Vec<u8>,
        inicio: usize,
        quantos: usize,
    ) -> Resultado<()> {
        let alvo = saida
            .len()
            .checked_add(quantos)
            .ok_or(Erro::GrandeDemaisParaEsteAlvo)?;
        let m = &mut self.modelo;
        while saida.len() < alvo {
            let pos = (saida.len() - inicio) as u64;
            let pe = m.pos_estado(pos);
            let st = m.estado;
            if rc.bit(&mut m.e_casamento[st][pe])? == 0 {
                let anterior = if saida.len() > inicio {
                    saida[saida.len() - 1]
                } else {
                    0
                };
                let base = m.base_literal(pos, anterior);
                let probs = &mut m.literais[base..base + 0x300];
                let mut s = 1usize;
                if st >= 7 {
                    let d = m.reps[0] as usize + 1;
                    if d > saida.len() - inicio {
                        return Err(Erro::Corrompido("distancia LZMA alem do dicionario"));
                    }
                    let mut casado = saida[saida.len() - d] as usize;
                    while s < 0x100 {
                        let bit_c = (casado >> 7) & 1;
                        casado <<= 1;
                        let b = rc.bit(&mut probs[0x100 + (bit_c << 8) + s])? as usize;
                        s = (s << 1) | b;
                        if b != bit_c {
                            break;
                        }
                    }
                }
                while s < 0x100 {
                    s = (s << 1) | rc.bit(&mut probs[s])? as usize;
                }
                saida.push(s as u8);
                m.apos_literal();
                continue;
            }

            let compr;
            if rc.bit(&mut m.e_rep[st])? == 0 {
                compr = ler_compr(rc, &mut m.compr, pe)?;
                m.reps = [0, m.reps[0], m.reps[1], m.reps[2]];
                m.apos_casamento();
                let fenda = rc.arvore(&mut m.fenda[estado_de_compr(compr)], 6)?;
                let dist = if fenda < 4 {
                    fenda
                } else {
                    let n = (fenda >> 1) - 1;
                    let base = (2 | (fenda & 1)) << n;
                    if fenda < FIM_DO_MODELO_DE_POS {
                        let i = (base - fenda) as usize;
                        base + rc.arvore_reversa(&mut m.especial[i..], n)?
                    } else {
                        base + (rc.diretos(n - 4)? << 4)
                            + rc.arvore_reversa(&mut m.alinhamento, 4)?
                    }
                };
                if dist == u32::MAX {
                    return Err(Erro::Corrompido(
                        "marca de fim do LZMA antes do tamanho declarado",
                    ));
                }
                m.reps[0] = dist;
            } else {
                if rc.bit(&mut m.e_rep_g0[st])? == 0 {
                    if rc.bit(&mut m.e_rep0_longo[st][pe])? == 0 {
                        if saida.len() <= inicio {
                            return Err(Erro::Corrompido("repeticao antes de haver dado"));
                        }
                        let d = m.reps[0] as usize + 1;
                        if d > saida.len() - inicio {
                            return Err(Erro::Corrompido("distancia LZMA alem do dicionario"));
                        }
                        let b = saida[saida.len() - d];
                        saida.push(b);
                        m.apos_rep_curto();
                        continue;
                    }
                } else {
                    let dist;
                    if rc.bit(&mut m.e_rep_g1[st])? == 0 {
                        dist = m.reps[1];
                    } else {
                        if rc.bit(&mut m.e_rep_g2[st])? == 0 {
                            dist = m.reps[2];
                        } else {
                            dist = m.reps[3];
                            m.reps[3] = m.reps[2];
                        }
                        m.reps[2] = m.reps[1];
                    }
                    m.reps[1] = m.reps[0];
                    m.reps[0] = dist;
                }
                compr = ler_compr(rc, &mut m.compr_rep, pe)?;
                m.apos_rep();
            }

            let d = m.reps[0] as usize + 1;
            if d > saida.len() - inicio {
                return Err(Erro::Corrompido("distancia LZMA alem do dicionario"));
            }
            if saida.len() + compr > alvo {
                return Err(Erro::Corrompido(
                    "casamento LZMA passa do tamanho declarado",
                ));
            }
            let mut de = saida.len() - d;
            for _ in 0..compr {
                let b = saida[de];
                saida.push(b);
                de += 1;
            }
        }
        Ok(())
    }
}

/// LZMA puro (metodo 030101 do 7z): 5 bytes de propriedade, tamanho conhecido.
pub fn decodificar_lzma(
    props5: &[u8],
    dados: &[u8],
    tamanho: usize,
    saida: &mut Vec<u8>,
) -> Resultado<()> {
    if props5.len() != 5 {
        return Err(Erro::Corrompido("propriedades LZMA nao tem 5 bytes"));
    }
    let props = Props::do_byte(props5[0]).ok_or(Erro::Corrompido("byte lc/lp/pb invalido"))?;
    let mut dec = DecodificadorLzma::novo(props)?;
    if tamanho == 0 {
        return Ok(());
    }
    let mut rc = Decodificador::novo(dados)?;
    let inicio = saida.len();
    dec.decodificar(&mut rc, saida, inicio, tamanho)
}
