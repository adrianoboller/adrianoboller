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
    decodificar_trecho(p, dados, tamanho, saida, None, true)
}

/// Um trecho do fluxo LZMA2 que comeca num reinicio de dicionario (um
/// [`Corte`]). `props` sao as ultimas propriedades vistas ANTES do trecho --
/// um trecho que abre com pedaco cru pode herda-las, e decodifica-lo sozinho
/// sem elas recusaria. `com_fim`: o trecho termina no byte de fim (o ultimo);
/// senao termina no fim da fatia.
pub fn decodificar_trecho(
    p: u8,
    dados: &[u8],
    tamanho: usize,
    saida: &mut Vec<u8>,
    props: Option<Props>,
    com_fim: bool,
) -> Resultado<()> {
    if p > 40 {
        return Err(Erro::Corrompido("propriedade do LZMA2 acima de 40"));
    }
    let base = saida.len();
    let alvo = base
        .checked_add(tamanho)
        .ok_or(Erro::GrandeDemaisParaEsteAlvo)?;
    let mut i = 0usize;
    let mut dec: Option<DecodificadorLzma> = match props {
        Some(pr) => Some(DecodificadorLzma::novo(pr)?),
        None => None,
    };
    let mut inicio = base;
    let mut precisa_dict = true;
    let mut precisa_props = props.is_none();
    let mut precisa_estado = true;
    loop {
        if !com_fim && i == dados.len() {
            break;
        }
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

/// Como [`decodificar_lzma2`], com ate `fios` fios quando o fluxo tem mais de
/// um trecho independente (o que a compactacao em blocos grava). Fluxo de um
/// trecho so -- o que um fio, ou o 7-Zip abaixo de 64 MiB, gravou -- nao tem
/// como se dividir: e o limite do proprio LZMA, e segue num fio.
///
/// Cada fio decodifica um trecho num `Vec` proprio e o copia para a fatia
/// dele em `saida`: o pico e a saida mais um trecho por fio, e nao o dobro.
/// O resultado e identico ao de um fio (ha teste), e qualquer trecho com
/// erro faz o todo recusar.
pub fn decodificar_lzma2_em_fios(
    p: u8,
    dados: &[u8],
    tamanho: usize,
    saida: &mut Vec<u8>,
    fios: usize,
) -> Resultado<()> {
    #[cfg(feature = "std")]
    if fios > 1 {
        let (cortes, total) = cortes_lzma2(dados)?;
        if cortes.len() > 1 {
            if total != tamanho {
                return Err(Erro::Corrompido(
                    "LZMA2 produz outro tamanho que o declarado",
                ));
            }
            return em_paralelo(p, dados, tamanho, saida, fios, &cortes);
        }
    }
    let _ = fios;
    decodificar_lzma2(p, dados, tamanho, saida)
}

#[cfg(feature = "std")]
fn em_paralelo(
    p: u8,
    dados: &[u8],
    tamanho: usize,
    saida: &mut Vec<u8>,
    fios: usize,
    cortes: &[Corte],
) -> Resultado<()> {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let base = saida.len();
    saida.resize(base + tamanho, 0);
    // As fatias da saida, uma por trecho, na ordem: disjuntas, entao cada fio
    // escreve na sua sem trava nenhuma sobre os bytes.
    let mut fatias: Vec<Mutex<&mut [u8]>> = Vec::with_capacity(cortes.len());
    let mut resto = &mut saida[base..];
    for (k, c) in cortes.iter().enumerate() {
        let fim = cortes.get(k + 1).map_or(tamanho, |d| d.na_saida);
        let (esta, depois) = resto.split_at_mut(fim - c.na_saida);
        fatias.push(Mutex::new(esta));
        resto = depois;
    }
    let proximo = AtomicUsize::new(0);
    let falha: Mutex<Option<Erro>> = Mutex::new(None);
    let n = fios.min(cortes.len());
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| loop {
                let k = proximo.fetch_add(1, Ordering::Relaxed);
                let Some(c) = cortes.get(k) else { break };
                if falha.lock().map(|f| f.is_some()).unwrap_or(true) {
                    break;
                }
                let ultimo = k + 1 == cortes.len();
                let ate = cortes.get(k + 1).map_or(dados.len(), |d| d.no_fluxo);
                let Ok(mut destino) = fatias[k].lock() else {
                    break;
                };
                let mut v = Vec::new();
                let r = v
                    .try_reserve_exact(destino.len())
                    .map_err(|_| Erro::Teto("memoria para descompactar"))
                    .and_then(|_| {
                        decodificar_trecho(
                            p,
                            &dados[c.no_fluxo..ate],
                            destino.len(),
                            &mut v,
                            c.props,
                            ultimo,
                        )
                    });
                match r {
                    Ok(()) => destino.copy_from_slice(&v),
                    Err(e) => {
                        if let Ok(mut f) = falha.lock() {
                            f.get_or_insert(e);
                        }
                        break;
                    }
                }
            });
        }
    });
    match falha.into_inner() {
        Ok(None) => Ok(()),
        Ok(Some(e)) => {
            saida.truncate(base);
            Err(e)
        }
        Err(_) => {
            saida.truncate(base);
            Err(Erro::Corrompido("fio da descompactacao caiu"))
        }
    }
}

/// Onde o fluxo LZMA2 recomeca o dicionario: o deslocamento no fluxo, o
/// deslocamento na SAIDA e as propriedades vigentes antes dali. Cada corte
/// comeca um trecho que se decodifica sem o anterior -- e o que deixa
/// descompactar em paralelo o que foi compactado em blocos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corte {
    /// Byte do fluxo onde o trecho comeca.
    pub no_fluxo: usize,
    /// Byte da saida onde o trecho comeca.
    pub na_saida: usize,
    /// Propriedades em vigor antes do trecho (o ultimo byte de props lido).
    pub props: Option<Props>,
}

/// Varre SO os cabecalhos dos pedacos, sem decodificar nada, e devolve os
/// cortes (o primeiro e sempre o inicio) e o tamanho total da saida. Custa
/// microssegundos por pedaco de ate 2 MiB. Recusa o que o decodificador
/// recusaria no cabecalho: truncado, byte de controle invalido, props ruins.
pub fn cortes_lzma2(dados: &[u8]) -> Resultado<(Vec<Corte>, usize)> {
    let mut cortes = Vec::new();
    let mut i = 0usize;
    let mut saida = 0usize;
    let mut props: Option<Props> = None;
    loop {
        let c = *dados
            .get(i)
            .ok_or(Erro::Corrompido("LZMA2 sem o byte de fim"))?;
        if c == 0 {
            break;
        }
        let (desc, comp, reinicia, novas) = if c == 1 || c == 2 {
            let n = ler_u16(dados, i + 1)? as usize + 1;
            (n, n, c == 1, None)
        } else if c >= 0x80 {
            let modo = (c >> 5) & 3;
            let desc = (((c as usize) & 0x1F) << 16) + ler_u16(dados, i + 1)? as usize + 1;
            let comp = ler_u16(dados, i + 3)? as usize + 1;
            let novas = if modo >= 2 {
                let b = *dados
                    .get(i + 5)
                    .ok_or(Erro::Corrompido("LZMA2 truncado nas propriedades"))?;
                Some(Props::do_byte(b).ok_or(Erro::Corrompido("propriedades LZMA2 invalidas"))?)
            } else {
                None
            };
            (desc, comp, modo == 3, novas)
        } else {
            return Err(Erro::Corrompido("byte de controle do LZMA2 invalido"));
        };
        if reinicia {
            cortes.push(Corte {
                no_fluxo: i,
                na_saida: saida,
                props,
            });
        } else if cortes.is_empty() {
            return Err(Erro::Corrompido("LZMA2 comeca sem reiniciar o dicionario"));
        }
        if novas.is_some() {
            props = novas;
        }
        let cab = if c < 0x80 {
            3
        } else if novas.is_some() {
            6
        } else {
            5
        };
        i = i
            .checked_add(cab + comp)
            .filter(|f| *f <= dados.len())
            .ok_or(Erro::Corrompido("pedaco do LZMA2 truncado"))?;
        saida = saida
            .checked_add(desc)
            .ok_or(Erro::GrandeDemaisParaEsteAlvo)?;
    }
    Ok((cortes, saida))
}

fn ler_u16(d: &[u8], i: usize) -> Resultado<u16> {
    match d.get(i..i + 2) {
        Some(b) => Ok(u16::from_be_bytes([b[0], b[1]])),
        None => Err(Erro::Corrompido("cabecalho de pedaco LZMA2 truncado")),
    }
}

#[cfg(all(test, feature = "std"))]
mod testes {
    use super::*;
    use crate::lzma::{blocos_lzma2, codificar_bloco_lzma2, juntar_blocos_lzma2, Nivel};

    /// Texto que comprime, com um trecho aleatorio no meio: o bloco dele sai
    /// em pedacos CRUS, e o trecho seguinte tem de se decodificar sozinho.
    fn amostra() -> Vec<u8> {
        let mut v = Vec::new();
        let mut x = 99u32;
        while v.len() < 400_000 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            if (200_000..280_000).contains(&v.len()) {
                v.push((x >> 23) as u8);
            } else {
                v.extend_from_slice(alloc::format!("campo {} da tabela\n", x >> 20).as_bytes());
            }
        }
        v
    }

    fn em_blocos(d: &[u8], bloco: usize) -> (u8, Vec<u8>) {
        let nivel = Nivel::de(5);
        let blocos: Vec<Vec<u8>> = blocos_lzma2(d, bloco)
            .map(|b| codificar_bloco_lzma2(b, nivel))
            .collect();
        juntar_blocos_lzma2(d.len(), bloco, nivel, &blocos)
    }

    #[test]
    fn em_fios_da_o_mesmo_que_um_fio_e_acha_um_corte_por_bloco() {
        let d = amostra();
        let (p, fluxo) = em_blocos(&d, 1 << 16);
        let (cortes, total) = cortes_lzma2(&fluxo).unwrap();
        assert_eq!(total, d.len());
        assert_eq!(cortes.len(), d.len().div_ceil(1 << 16));
        // O primeiro trecho nao tem passado; os outros herdam as props do
        // anterior (o corte as registra para um trecho que abra cru).
        assert_eq!(cortes[0].props, None);
        assert!(cortes[1..].iter().all(|c| c.props == Some(Props::PADRAO)));
        for fios in [1, 2, 3, 8] {
            let mut v = Vec::new();
            decodificar_lzma2_em_fios(p, &fluxo, d.len(), &mut v, fios).unwrap();
            assert!(v == d, "{fios} fios deram outra saida");
        }
    }

    /// Um byte trocado num trecho do MEIO: o todo recusa, e a saida nao fica
    /// com meio conteudo.
    #[test]
    fn byte_estragado_num_trecho_do_meio_recusa_o_todo() {
        let d = amostra();
        let (p, mut fluxo) = em_blocos(&d, 1 << 16);
        let (cortes, _) = cortes_lzma2(&fluxo).unwrap();
        let alvo = cortes[3].no_fluxo + 40;
        fluxo[alvo] ^= 0x5A;
        let mut v = Vec::new();
        assert!(decodificar_lzma2_em_fios(p, &fluxo, d.len(), &mut v, 4).is_err());
        assert!(v.is_empty(), "a saida ficou com meio conteudo");
    }

    /// Tamanho declarado que nao fecha com os cabecalhos: recusa antes de
    /// subir fio.
    #[test]
    fn tamanho_declarado_diferente_recusa() {
        let d = amostra();
        let (p, fluxo) = em_blocos(&d, 1 << 16);
        let mut v = Vec::new();
        assert!(decodificar_lzma2_em_fios(p, &fluxo, d.len() - 1, &mut v, 4).is_err());
    }
}
