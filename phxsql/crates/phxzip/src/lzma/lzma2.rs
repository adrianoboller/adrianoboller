//! Decodificador LZMA2: pedacos de LZMA e de dado cru, com os quatro niveis de
//! reinicio no byte de controle.

use alloc::vec::Vec;

use super::dec::DecodificadorLzma;
use super::faixa::Decodificador;
use super::Props;
use crate::erro::{Erro, Resultado};

/// Decodifica um fluxo LZMA2 inteiro, que tem de produzir exatamente
/// `tamanho` bytes. `p` e o byte de propriedade (dicionario) do coder.
pub fn decodificar_lzma2(
    p: u8,
    dados: &[u8],
    tamanho: usize,
    saida: &mut Vec<u8>,
) -> Resultado<()> {
    if p > 40 {
        return Err(Erro::Corrompido("propriedade do LZMA2 acima de 40"));
    }
    let base = saida.len();
    let alvo = base
        .checked_add(tamanho)
        .ok_or(Erro::GrandeDemaisParaEsteAlvo)?;
    let mut i = 0usize;
    let mut dec: Option<DecodificadorLzma> = None;
    let mut inicio = base;
    let mut precisa_dict = true;
    let mut precisa_props = true;
    let mut precisa_estado = true;
    loop {
        let c = *dados
            .get(i)
            .ok_or(Erro::Corrompido("LZMA2 sem o byte de fim"))?;
        i += 1;
        if c == 0 {
            break;
        }
        if c == 1 || c == 2 {
            if c == 1 {
                inicio = saida.len();
                precisa_dict = false;
            } else if precisa_dict {
                return Err(Erro::Corrompido("LZMA2 comeca sem reiniciar o dicionario"));
            }
            let n = ler_u16(dados, i)? as usize + 1;
            i += 2;
            let fatia = dados
                .get(i..i + n)
                .ok_or(Erro::Corrompido("pedaco cru do LZMA2 truncado"))?;
            if saida.len() + n > alvo {
                return Err(Erro::Corrompido("LZMA2 produz mais que o declarado"));
            }
            saida.extend_from_slice(fatia);
            i += n;
            continue;
        }
        if c < 0x80 {
            return Err(Erro::Corrompido("byte de controle do LZMA2 invalido"));
        }
        let modo = (c >> 5) & 3;
        let desc = (((c as usize) & 0x1F) << 16) + ler_u16(dados, i)? as usize + 1;
        let comp = ler_u16(dados, i + 2)? as usize + 1;
        i += 4;
        if modo == 3 {
            inicio = saida.len();
            precisa_dict = false;
        } else if precisa_dict {
            return Err(Erro::Corrompido("LZMA2 comeca sem reiniciar o dicionario"));
        }
        let props = if modo >= 2 {
            let b = *dados
                .get(i)
                .ok_or(Erro::Corrompido("LZMA2 truncado nas propriedades"))?;
            i += 1;
            let pr = Props::do_byte(b).ok_or(Erro::Corrompido("propriedades LZMA2 invalidas"))?;
            if pr.lc + pr.lp > 4 {
                return Err(Erro::Corrompido("LZMA2 exige lc + lp <= 4"));
            }
            precisa_props = false;
            Some(pr)
        } else {
            if precisa_props {
                return Err(Erro::Corrompido("pedaco LZMA2 sem propriedades antes"));
            }
            None
        };
        if modo >= 1 {
            match dec.as_mut() {
                Some(d) => d.reiniciar(props),
                None => dec = Some(DecodificadorLzma::novo(props.unwrap_or(Props::PADRAO))?),
            }
            precisa_estado = false;
        } else if precisa_estado {
            return Err(Erro::Corrompido("pedaco LZMA2 sem reiniciar o estado"));
        }
        if saida.len() + desc > alvo {
            return Err(Erro::Corrompido("LZMA2 produz mais que o declarado"));
        }
        let fatia = dados
            .get(i..i + comp)
            .ok_or(Erro::Corrompido("pedaco LZMA do LZMA2 truncado"))?;
        let mut rc = Decodificador::novo(fatia)?;
        let d = dec.as_mut().ok_or(Erro::Corrompido("LZMA2 sem estado"))?;
        d.decodificar(&mut rc, saida, inicio, desc)?;
        if rc.consumidos() != comp || !rc.terminou_limpo() {
            return Err(Erro::Corrompido(
                "pedaco LZMA do LZMA2 nao fecha no tamanho comprimido",
            ));
        }
        i += comp;
    }
    if saida.len() != alvo {
        return Err(Erro::Corrompido("LZMA2 produziu menos que o declarado"));
    }
    Ok(())
}

fn ler_u16(d: &[u8], i: usize) -> Resultado<u16> {
    match d.get(i..i + 2) {
        Some(b) => Ok(u16::from_be_bytes([b[0], b[1]])),
        None => Err(Erro::Corrompido("cabecalho de pedaco LZMA2 truncado")),
    }
}
