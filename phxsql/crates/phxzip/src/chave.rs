//! 7zAES (`06F10701`): a derivacao da chave, as propriedades do coder e o
//! vetor de inicio sintetico do escritor.
//!
//! # A derivacao, como foi entendida
//!
//! Lida em `CPP/7zip/Crypto/7zAes.cpp` do 7-Zip 26.03, que e LGPL -- **so
//! leitura**, nenhuma linha copiada. A construcao, nas nossas palavras: a chave
//! AES-256 e o SHA-256 de UMA corrente feita de `2^ciclos` repeticoes de
//! `sal || senha || contador`, onde a senha vai em UTF-16LE (e o que o 7-Zip
//! entrega ao coder) e o contador tem 8 bytes little-endian, de 0 em diante.
//! Com `ciclos = 0x3F` nao ha hash: a chave e `sal || senha` completada com zero.
//!
//! O custo e o numero de compressoes do SHA-256 -- ver [`compressoes`]. Com o
//! padrao do 7-Zip (19) e uma senha de 16 caracteres sao 327.681, e isso e o
//! que um ESP32 sente. A senha do `.phz` e publica, entao rodada nenhuma compra
//! protecao ali: o escritor deixa escolher menos, e o leitor aceita o que o
//! 7-Zip aceita (ate 24, `k_NumCyclesPower_Supported_MAX` em `7zAes.cpp:27`).
//!
//! # O vetor de inicio (IV) e sintetico, e por que
//!
//! O 7-Zip sorteia o IV. Esta crate nao pede numero aleatorio ao sistema -- ela
//! roda em microcontrolador, onde nem sempre ha um --, entao o IV e
//! `HMAC-SHA256(k_iv, rotulo || dados)`, cortado em 16 bytes, com
//! `k_iv = SHA-256("PhxZip IV" || chave)`. Duas propriedades decidem:
//!
//! * **Depende da chave.** Um IV que fosse so o hash do conteudo deixaria quem
//!   NAO tem a senha confirmar um palpite do conteudo comparando hashes. Com a
//!   chave dentro, nao.
//! * **O que se perde e medido:** o mesmo conteudo com a mesma senha da o
//!   mesmo arquivo -- quem ve dois `.phz` sabe se sao iguais, e nada alem.
//!   E a mesma troca do SIV (RFC 5297), e o ganho e o arquivo reproduzivel.

use alloc::vec::Vec;

use phxsql_core::hash::{hmac_sha256, sha256, Sha256};

use crate::aes::{BLOCO, CHAVE};
use crate::erro::Erro;

/// O identificador do coder 7zAES no 7z.
pub const ID_7ZAES: u64 = 0x06F1_0701;
/// O `NumCyclesPower` que o 7-Zip grava (`7zAes.cpp:236`).
pub const CICLOS_PADRAO: u8 = 19;
/// O maior que o 7-Zip le (`7zAes.cpp:27`).
pub const CICLOS_MAXIMO: u8 = 24;
/// Valor especial: sem hash nenhum.
const SEM_HASH: u8 = 0x3F;

/// As propriedades de um coder 7zAES.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Props {
    pub ciclos: u8,
    pub sal: Vec<u8>,
    pub iv: [u8; BLOCO],
}

/// Le as propriedades do coder. O byte 0 traz os ciclos (6 bits) e dois bits
/// que dizem se ha sal e IV; o byte 1 traz os tamanhos menos um.
pub(crate) fn ler_props(props: &[u8], teto_ciclos: u8) -> Result<Props, Erro> {
    let mut p = Props {
        ciclos: 0,
        sal: Vec::new(),
        iv: [0; BLOCO],
    };
    let Some(&b0) = props.first() else {
        return Ok(p);
    };
    p.ciclos = b0 & 0x3F;
    if b0 & 0xC0 == 0 {
        if props.len() != 1 {
            return Err(Erro::Estrutura(
                "propriedades do 7zAES com bytes a mais".into(),
            ));
        }
    } else {
        let b1 = *props
            .get(1)
            .ok_or_else(|| Erro::Estrutura("propriedades do 7zAES cortadas".into()))?;
        let sal = ((b0 >> 7) & 1) as usize + (b1 >> 4) as usize;
        let iv = ((b0 >> 6) & 1) as usize + (b1 & 0x0F) as usize;
        if props.len() != 2 + sal + iv {
            return Err(Erro::Estrutura(
                "propriedades do 7zAES com tamanho que nao fecha".into(),
            ));
        }
        p.sal = props[2..2 + sal].to_vec();
        p.iv[..iv].copy_from_slice(&props[2 + sal..2 + sal + iv]);
    }
    if p.ciclos != SEM_HASH && p.ciclos > teto_ciclos {
        return Err(Erro::CiclosDemais {
            pedidos: p.ciclos,
            teto: teto_ciclos,
        });
    }
    Ok(p)
}

/// As propriedades que o escritor grava: sem sal (como o 7-Zip) e IV de 16.
pub(crate) fn escrever_props(ciclos: u8, iv: &[u8; BLOCO]) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 + BLOCO);
    v.push((ciclos & 0x3F) | 0x40);
    v.push(0x0F);
    v.extend_from_slice(iv);
    v
}

/// A senha como o 7-Zip a entrega ao coder: UTF-16LE, sem terminador.
pub(crate) fn senha_utf16(senha: &str) -> Vec<u8> {
    senha.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

/// Quantas compressoes de bloco do SHA-256 a derivacao faz -- o numero que
/// decide o tempo num microcontrolador. Cada rodada tem `sal + senha + 8`
/// bytes; o SHA-256 comprime de 64 em 64, e o fim acrescenta 9 bytes de
/// preenchimento no minimo.
pub fn compressoes(ciclos: u8, bytes_da_senha_utf16: usize, bytes_do_sal: usize) -> u64 {
    if ciclos == SEM_HASH {
        return 0;
    }
    let por_rodada = (bytes_do_sal + bytes_da_senha_utf16 + 8) as u64;
    let total = por_rodada << ciclos;
    (total + 9).div_ceil(64)
}

/// Deriva a chave AES-256.
pub(crate) fn derivar(senha16: &[u8], sal: &[u8], ciclos: u8) -> [u8; CHAVE] {
    let mut chave = [0u8; CHAVE];
    if ciclos == SEM_HASH {
        for (d, s) in chave.iter_mut().zip(sal.iter().chain(senha16.iter())) {
            *d = *s;
        }
        return chave;
    }
    // Rodadas em lote: o SHA-256 recebe varias de uma vez, e a corrente e a
    // mesma que recebe-las uma a uma -- so com menos chamadas.
    const LOTE: u64 = 64;
    let por_rodada = sal.len() + senha16.len() + 8;
    let mut lote = Vec::with_capacity(por_rodada * LOTE as usize);
    let mut h = Sha256::novo();
    let total = 1u64 << ciclos;
    let mut i = 0u64;
    while i < total {
        let fim = (i + LOTE).min(total);
        lote.clear();
        for contador in i..fim {
            lote.extend_from_slice(sal);
            lote.extend_from_slice(senha16);
            lote.extend_from_slice(&contador.to_le_bytes());
        }
        h.atualizar(&lote);
        i = fim;
    }
    chave.copy_from_slice(&h.finalizar());
    chave
}

/// Guarda de chaves ja derivadas no mesmo arquivo: o cabecalho e cada bloco
/// trazem as proprias propriedades, e quase sempre com o mesmo sal e os mesmos
/// ciclos -- derivar de novo custaria outra vez as 2^19 rodadas.
///
/// # E o teto de derivacoes, e por que ele mora aqui
///
/// O cache so poupa quem REPETE sal e ciclos. Um arquivo hostil com um sal
/// diferente em cada bloco fura o cache e cobra uma derivacao inteira por
/// bloco -- o amplificador que o parecer SEC de 24/09/2026 apontou (pedido
/// 471). Este e o unico lugar onde uma derivacao acontece, entao e aqui que
/// ela se conta: a derivacao que passaria do teto e recusada ANTES de rodar.
/// O 7-Zip grava sal vazio em todo bloco, e um arquivo inteiro dele custa UMA.
pub(crate) struct Chaves {
    itens: Vec<(u8, Vec<u8>, [u8; CHAVE])>,
    teto: u32,
}

impl Chaves {
    /// Um cache que deriva no maximo `teto` chaves distintas.
    pub fn com_teto(teto: u32) -> Chaves {
        Chaves {
            itens: Vec::new(),
            teto,
        }
    }

    pub fn obter(&mut self, senha16: &[u8], sal: &[u8], ciclos: u8) -> Result<[u8; CHAVE], Erro> {
        if let Some((_, _, k)) = self
            .itens
            .iter()
            .find(|(c, s, _)| *c == ciclos && s.as_slice() == sal)
        {
            return Ok(*k);
        }
        if self.itens.len() as u64 >= self.teto as u64 {
            return Err(Erro::GrandeDemais {
                oque: "derivacoes de chave",
                declarado: self.itens.len() as u64 + 1,
                teto: self.teto as u64,
            });
        }
        let k = derivar(senha16, sal, ciclos);
        self.itens.push((ciclos, sal.to_vec(), k));
        Ok(k)
    }

    /// Quantas derivacoes ja rodaram -- o trabalho de CPU feito, para o teste
    /// medir o dano e nao so o nome do erro.
    #[cfg(test)]
    pub fn derivadas(&self) -> usize {
        self.itens.len()
    }
}

/// O IV sintetico descrito no topo do modulo.
pub(crate) fn iv_sintetico(chave: &[u8; CHAVE], rotulo: &[u8], dados: &[u8]) -> [u8; BLOCO] {
    let mut semente = Vec::with_capacity(9 + CHAVE);
    semente.extend_from_slice(b"PhxZip IV");
    semente.extend_from_slice(chave);
    let k_iv = sha256(&semente);
    let mut msg = Vec::with_capacity(rotulo.len() + dados.len());
    msg.extend_from_slice(rotulo);
    msg.extend_from_slice(dados);
    let mac = hmac_sha256(&k_iv, &msg);
    let mut iv = [0u8; BLOCO];
    iv.copy_from_slice(&mac[..BLOCO]);
    iv
}

#[cfg(test)]
mod testes {
    use super::*;
    use alloc::vec;

    /// Vetor calculado FORA daqui, pelo `hashlib` do Python, a partir da
    /// descricao da construcao:
    /// `sha256(b"".join(pw + i.to_bytes(8, "little") for i in range(2**4)))`
    /// com `pw = "senha-inventada".encode("utf-16-le")`. Nao e vetor oficial --
    /// o 7zAES nao tem um --; confere que o codigo faz o que o texto diz. Quem
    /// confere que o texto diz o que o 7-Zip faz e a interoperabilidade com os
    /// arquivos que o proprio 7-Zip gravou (`tests/phz.rs`).
    #[test]
    fn derivacao_confere_com_a_conta_feita_fora() {
        let k = derivar(&senha_utf16("senha-inventada"), &[], 4);
        assert_eq!(
            phxsql_core::hash::para_hex(&k),
            "4f682a6bdfdcb056c17faa577837fff059cca4e076f2c1d2732a6737bc052c20"
        );
    }

    #[test]
    fn sem_hash_e_sal_mais_senha_completado_com_zero() {
        let k = derivar(&[1, 2, 3], &[9], SEM_HASH);
        let mut esperado = [0u8; CHAVE];
        esperado[..4].copy_from_slice(&[9, 1, 2, 3]);
        assert_eq!(k, esperado);
    }

    #[test]
    fn props_vao_e_voltam() {
        let iv = [7u8; BLOCO];
        let p = ler_props(&escrever_props(19, &iv), CICLOS_MAXIMO).unwrap();
        assert_eq!(
            p,
            Props {
                ciclos: 19,
                sal: vec![],
                iv
            }
        );
    }

    /// O que o 7-Zip 23.01 gravou (`7z a -p... -mhe=on`): 0x53 = 19 | IV
    /// presente, 0x0F = IV de 16 bytes.
    #[test]
    fn le_as_props_que_o_7zip_grava() {
        let mut props = vec![0x53, 0x0f];
        props.extend_from_slice(&[0xab; 16]);
        let p = ler_props(&props, CICLOS_MAXIMO).unwrap();
        assert_eq!((p.ciclos, p.sal.len(), p.iv), (19, 0, [0xab; 16]));
    }

    #[test]
    fn props_hostis_sao_recusadas_com_nome() {
        // Tamanho que nao fecha.
        assert!(matches!(
            ler_props(&[0x53, 0x0f, 1, 2], 24),
            Err(Erro::Estrutura(_))
        ));
        // Cortada no segundo byte.
        assert!(matches!(ler_props(&[0x53], 24), Err(Erro::Estrutura(_))));
        // Ciclos acima do teto: a bomba de CPU.
        assert_eq!(
            ler_props(&[30], 24),
            Err(Erro::CiclosDemais {
                pedidos: 30,
                teto: 24
            })
        );
    }

    /// O numero que o relatorio do IoT cita, conferido pela formula: 16
    /// caracteres = 32 bytes em UTF-16, mais 8 do contador, vezes 2^19.
    #[test]
    fn compressoes_do_padrao() {
        assert_eq!(compressoes(19, 32, 0), (40u64 * 524_288 + 9).div_ceil(64));
        assert_eq!(compressoes(19, 32, 0), 327_681);
        assert_eq!(compressoes(SEM_HASH, 32, 0), 0);
    }

    /// O amplificador do parecer SEC: sal diferente por bloco fura o cache.
    /// Com teto 2, a terceira derivacao e recusada SEM rodar -- e o vermelho,
    /// com a conferencia tirada, conta quantas rodaram.
    #[test]
    fn sal_diferente_por_bloco_nao_multiplica_a_derivacao() {
        let senha = senha_utf16("senha-inventada");
        let mut ch = Chaves::com_teto(2);
        let mut recusadas = 0;
        for sal in 0..8u8 {
            match ch.obter(&senha, &[sal], 4) {
                Ok(_) => {}
                Err(Erro::GrandeDemais {
                    oque: "derivacoes de chave",
                    ..
                }) => recusadas += 1,
                Err(e) => panic!("veio {e:?}"),
            }
        }
        assert!(
            ch.derivadas() <= 2,
            "{} derivacoes rodaram com teto de 2 -- cada uma custa 2^ciclos SHA-256",
            ch.derivadas()
        );
        assert_eq!(recusadas, 6);
        // O que ja foi derivado continua servindo, sem contar de novo.
        assert!(ch.obter(&senha, &[0], 4).is_ok());
    }

    #[test]
    fn o_iv_depende_da_chave_e_do_dado() {
        let a = iv_sintetico(&[1; CHAVE], b"x", b"conteudo");
        assert_ne!(a, iv_sintetico(&[2; CHAVE], b"x", b"conteudo"));
        assert_ne!(a, iv_sintetico(&[1; CHAVE], b"x", b"conteudO"));
        assert_ne!(a, iv_sintetico(&[1; CHAVE], b"y", b"conteudo"));
        assert_eq!(a, iv_sintetico(&[1; CHAVE], b"x", b"conteudo"));
    }
}
