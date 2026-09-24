//! O escritor do 7z: entradas (arquivos e pastas), compressao, 7zAES e o
//! cabecalho cifrado.
//!
//! # As escolhas, e o motivo de cada uma
//!
//! * **Um bloco por arquivo (nao solido).** O 7-Zip junta tudo num bloco so por
//!   padrao, e ganha compressao entre arquivos parecidos. Aqui cada arquivo e o
//!   proprio bloco, porque a web e o terminal vao extrair UMA entrada de um
//!   arquivo grande -- e num bloco solido isso custa decodificar tudo o que vem
//!   antes dela. O formato preve os dois; o 7-Zip le os dois.
//! * **CRC do dado cifrado.** O formato tem o campo (`kCRC` no `PackInfo`) e o
//!   7-Zip nao o preenche. Preenchido, e ele que deixa o leitor afirmar «senha
//!   errada» em vez de «senha errada OU arquivo corrompido» (`erro.rs`).
//! * **Cabecalho cifrado por padrao** quando ha senha (o `-mhe=on` do 7-Zip):
//!   esconde os nomes, e a senha errada aparece ja no cabecalho, antes de
//!   qualquer conteudo.
//! * **O cabecalho nao e comprimido.** Para as poucas entradas de um `.phz` ou
//!   de um backup, comprimir o cabecalho nao paga o codigo -- e o 7-Zip 23.01
//!   faz o mesmo em arquivo pequeno (medido: `7z a -mhe=on` grava o cabecalho so
//!   com o 7zAES).
//! * **Sem relogio e sem sorteio.** A data vem de quem chama; o IV e sintetico
//!   (`chave.rs`).

use alloc::string::String;
use alloc::vec::Vec;

use phxsql_core::crc::crc32;

use crate::aes::{self, Aes256, BLOCO};
use crate::chave::{self, CICLOS_MAXIMO, CICLOS_PADRAO, ID_7ZAES};
use crate::erro::Erro;
use crate::leitor::{conferir_nome, id, ASSINATURA, ATRIBUTO_PASTA, ID_COPIA, ID_LZMA2};
use crate::lzma_compressor;

/// Atributo «arquivo» do Windows, o `A` que o 7-Zip mostra.
const ATRIBUTO_ARQUIVO: u32 = 0x20;

/// Como o conteudo vai para dentro do arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metodo {
    /// Sem compressao (`Copy`).
    Copia,
    /// LZMA2, com o dicionario dado pelo byte de propriedade do 7z (ver
    /// [`crate::lzma_compressor::propriedade_do_dicionario`]).
    Lzma2,
}

/// O que se escolhe ao gravar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opcoes {
    pub metodo: Metodo,
    /// Sem senha, nada e cifrado.
    pub senha: Option<String>,
    /// O `NumCyclesPower` do 7zAES: 2^ciclos rodadas de SHA-256 para derivar a
    /// chave. O 7-Zip grava 19; aceita-se de 0 a 24.
    pub ciclos: u8,
    /// Cifrar tambem o cabecalho (nomes, tamanhos, datas). So vale com senha.
    pub cifrar_cabecalho: bool,
}

impl Default for Opcoes {
    fn default() -> Opcoes {
        Opcoes {
            metodo: Metodo::Lzma2,
            senha: None,
            ciclos: CICLOS_PADRAO,
            cifrar_cabecalho: true,
        }
    }
}

struct Item {
    nome: String,
    pasta: bool,
    conteudo: Vec<u8>,
    modificado: Option<u64>,
}

/// Monta um arquivo 7z na memoria.
pub struct Escritor {
    opcoes: Opcoes,
    itens: Vec<Item>,
}

/// O `UINT64` do 7z: tantos bits altos ligados no primeiro byte quantos bytes
/// vem depois, e o resto do valor nos bits que sobram dele.
pub(crate) fn escrever_numero(v: &mut Vec<u8>, n: u64) {
    for extras in 0..8u32 {
        if n < 1u64 << (7 * (extras + 1)) {
            let prefixo = !(0xFFu8 >> extras);
            v.push(prefixo | (n >> (8 * extras)) as u8);
            v.extend_from_slice(&n.to_le_bytes()[..extras as usize]);
            return;
        }
    }
    v.push(0xFF);
    v.extend_from_slice(&n.to_le_bytes());
}

fn escrever_bits(v: &mut Vec<u8>, bits: &[bool]) {
    let mut bytes = alloc::vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            bytes[i / 8] |= 0x80 >> (i % 8);
        }
    }
    v.extend_from_slice(&bytes);
}

/// Uma propriedade de `FilesInfo`: identificador, tamanho e o corpo.
fn propriedade(v: &mut Vec<u8>, ident: u64, corpo: &[u8]) {
    escrever_numero(v, ident);
    escrever_numero(v, corpo.len() as u64);
    v.extend_from_slice(corpo);
}

/// Um coder simples, como o `Folder` do 7z o descreve.
fn escrever_coder(v: &mut Vec<u8>, ident: u64, props: &[u8]) {
    let bytes = ident.to_be_bytes();
    let primeiro = bytes.iter().position(|b| *b != 0).unwrap_or(7);
    let ident = &bytes[primeiro..];
    let mut marca = ident.len() as u8;
    if !props.is_empty() {
        marca |= 0x20;
    }
    v.push(marca);
    v.extend_from_slice(ident);
    if !props.is_empty() {
        escrever_numero(v, props.len() as u64);
        v.extend_from_slice(props);
    }
}

/// Um bloco pronto: o que vai no disco e o que vai no cabecalho sobre ele.
struct BlocoGravado {
    empacotado: Vec<u8>,
    /// Coders na ordem declarada, com o tamanho da saida de cada um.
    coders: Vec<(u64, Vec<u8>, u64)>,
    crc: u32,
}

fn cifrar(chave: &[u8; 32], rotulo: &[u8], dado: &[u8], ciclos: u8) -> (Vec<u8>, Vec<u8>) {
    let iv = chave::iv_sintetico(chave, rotulo, dado);
    let mut buf = dado.to_vec();
    buf.resize(dado.len().div_ceil(BLOCO) * BLOCO, 0);
    // O tamanho ja foi completado ate fechar bloco; `cbc_cifrar` so recusa o
    // que nao fecha, entao aqui nao ha o que recusar.
    let _ = aes::cbc_cifrar(&Aes256::nova(chave), &iv, &mut buf);
    (buf, chave::escrever_props(ciclos, &iv))
}

fn escrever_fluxos(v: &mut Vec<u8>, pack_pos: u64, blocos: &[BlocoGravado], crc_no_bloco: bool) {
    escrever_numero(v, id::EMPACOTADOS);
    escrever_numero(v, pack_pos);
    escrever_numero(v, blocos.len() as u64);
    escrever_numero(v, id::TAMANHO);
    for b in blocos {
        escrever_numero(v, b.empacotado.len() as u64);
    }
    escrever_numero(v, id::CRC);
    v.push(1);
    for b in blocos {
        v.extend_from_slice(&crc32(&b.empacotado).to_le_bytes());
    }
    escrever_numero(v, id::FIM);

    escrever_numero(v, id::DESEMPACOTADOS);
    escrever_numero(v, id::BLOCO);
    escrever_numero(v, blocos.len() as u64);
    v.push(0);
    for b in blocos {
        escrever_numero(v, b.coders.len() as u64);
        for (ident, props, _) in &b.coders {
            escrever_coder(v, *ident, props);
        }
        // Com dois coders, a entrada do segundo (o descompressor) recebe a
        // saida do primeiro (o 7zAES) -- a mesma ligacao que o 7-Zip grava.
        if b.coders.len() == 2 {
            escrever_numero(v, 1);
            escrever_numero(v, 0);
        }
    }
    escrever_numero(v, id::TAMANHOS_DOS_CODERS);
    for b in blocos {
        for (_, _, saida) in &b.coders {
            escrever_numero(v, *saida);
        }
    }
    if crc_no_bloco {
        escrever_numero(v, id::CRC);
        v.push(1);
        for b in blocos {
            v.extend_from_slice(&b.crc.to_le_bytes());
        }
    }
    escrever_numero(v, id::FIM);

    if !crc_no_bloco {
        escrever_numero(v, id::SUBFLUXOS);
        escrever_numero(v, id::CRC);
        v.push(1);
        for b in blocos {
            v.extend_from_slice(&b.crc.to_le_bytes());
        }
        escrever_numero(v, id::FIM);
    }
    escrever_numero(v, id::FIM);
}

impl Escritor {
    /// Um escritor vazio. Recusa ciclos acima de 24, que o 7-Zip nao leria.
    pub fn novo(opcoes: Opcoes) -> Result<Escritor, Erro> {
        if opcoes.ciclos > CICLOS_MAXIMO {
            return Err(Erro::CiclosDemais {
                pedidos: opcoes.ciclos,
                teto: CICLOS_MAXIMO,
            });
        }
        Ok(Escritor {
            opcoes,
            itens: Vec::new(),
        })
    }

    fn acrescentar(
        &mut self,
        nome: &str,
        pasta: bool,
        conteudo: &[u8],
        modificado: Option<u64>,
    ) -> Result<(), Erro> {
        // A mesma conferencia do leitor: o escritor nao grava o que o leitor
        // recusaria abrir.
        let nome = conferir_nome(nome)?;
        if self.itens.iter().any(|i| i.nome == nome) {
            return Err(Erro::NomeRepetido(nome));
        }
        self.itens.push(Item {
            nome,
            pasta,
            conteudo: conteudo.to_vec(),
            modificado,
        });
        Ok(())
    }

    /// Acrescenta um arquivo. `modificado` e `FILETIME` (ver
    /// [`crate::unix_para_filetime`]).
    pub fn arquivo(
        &mut self,
        nome: &str,
        conteudo: &[u8],
        modificado: Option<u64>,
    ) -> Result<(), Erro> {
        self.acrescentar(nome, false, conteudo, modificado)
    }

    /// Acrescenta uma pasta.
    pub fn pasta(&mut self, nome: &str, modificado: Option<u64>) -> Result<(), Erro> {
        self.acrescentar(nome, true, &[], modificado)
    }

    /// Fecha o arquivo e devolve os bytes.
    pub fn terminar(self) -> Vec<u8> {
        let senha = self.opcoes.senha.as_deref();
        let chave = senha.map(|s| chave::derivar(&chave::senha_utf16(s), &[], self.opcoes.ciclos));
        let ciclos = self.opcoes.ciclos;

        let mut blocos = Vec::new();
        for (i, item) in self.itens.iter().enumerate() {
            if item.pasta || item.conteudo.is_empty() {
                continue;
            }
            let (metodo, comprimido) = match self.opcoes.metodo {
                Metodo::Copia => (None, item.conteudo.clone()),
                Metodo::Lzma2 => {
                    let prop = lzma_compressor::propriedade_do_dicionario(item.conteudo.len());
                    (
                        Some((ID_LZMA2, alloc::vec![prop])),
                        lzma_compressor::lzma2(&item.conteudo, prop),
                    )
                }
            };
            let mut coders = Vec::new();
            let empacotado = match &chave {
                Some(k) => {
                    let mut rotulo = Vec::from(&b"bloco "[..]);
                    rotulo.extend_from_slice(&(i as u64).to_le_bytes());
                    let (cifrado, props) = cifrar(k, &rotulo, &comprimido, ciclos);
                    coders.push((ID_7ZAES, props, comprimido.len() as u64));
                    cifrado
                }
                None => comprimido.clone(),
            };
            match metodo {
                Some((ident, props)) => coders.push((ident, props, item.conteudo.len() as u64)),
                None if chave.is_none() => {
                    coders.push((ID_COPIA, Vec::new(), item.conteudo.len() as u64))
                }
                None => {}
            }
            blocos.push(BlocoGravado {
                empacotado,
                coders,
                crc: crc32(&item.conteudo),
            });
        }

        let mut cab = Vec::new();
        escrever_numero(&mut cab, id::CABECALHO);
        if !blocos.is_empty() {
            escrever_numero(&mut cab, id::FLUXOS);
            escrever_fluxos(&mut cab, 0, &blocos, false);
        }
        if !self.itens.is_empty() {
            self.escrever_entradas(&mut cab);
        }
        escrever_numero(&mut cab, id::FIM);

        let mut corpo: Vec<u8> = Vec::new();
        for b in &blocos {
            corpo.extend_from_slice(&b.empacotado);
        }
        let cab_final = match (&chave, self.opcoes.cifrar_cabecalho) {
            (Some(k), true) => {
                let (cifrado, props) = cifrar(k, b"cabecalho", &cab, ciclos);
                let b = BlocoGravado {
                    empacotado: cifrado,
                    coders: alloc::vec![(ID_7ZAES, props, cab.len() as u64)],
                    crc: crc32(&cab),
                };
                let mut v = Vec::new();
                escrever_numero(&mut v, id::CABECALHO_CODIFICADO);
                escrever_fluxos(&mut v, corpo.len() as u64, core::slice::from_ref(&b), true);
                corpo.extend_from_slice(&b.empacotado);
                v
            }
            _ => cab,
        };

        let mut inicio = Vec::with_capacity(20);
        inicio.extend_from_slice(&(corpo.len() as u64).to_le_bytes());
        inicio.extend_from_slice(&(cab_final.len() as u64).to_le_bytes());
        inicio.extend_from_slice(&crc32(&cab_final).to_le_bytes());

        let mut arquivo = Vec::with_capacity(32 + corpo.len() + cab_final.len());
        arquivo.extend_from_slice(&ASSINATURA);
        arquivo.extend_from_slice(&[0, 4]);
        arquivo.extend_from_slice(&crc32(&inicio).to_le_bytes());
        arquivo.extend_from_slice(&inicio);
        arquivo.extend_from_slice(&corpo);
        arquivo.extend_from_slice(&cab_final);
        arquivo
    }

    fn escrever_entradas(&self, v: &mut Vec<u8>) {
        let n = self.itens.len();
        escrever_numero(v, id::ARQUIVOS);
        escrever_numero(v, n as u64);

        let vazios: Vec<bool> = self
            .itens
            .iter()
            .map(|i| i.pasta || i.conteudo.is_empty())
            .collect();
        if vazios.iter().any(|v| *v) {
            let mut corpo = Vec::new();
            escrever_bits(&mut corpo, &vazios);
            propriedade(v, id::FLUXO_VAZIO, &corpo);
            let arquivos_vazios: Vec<bool> = self
                .itens
                .iter()
                .filter(|i| i.pasta || i.conteudo.is_empty())
                .map(|i| !i.pasta)
                .collect();
            let mut corpo = Vec::new();
            escrever_bits(&mut corpo, &arquivos_vazios);
            propriedade(v, id::ARQUIVO_VAZIO, &corpo);
        }

        let mut nomes = alloc::vec![0u8];
        for item in &self.itens {
            for u in item.nome.encode_utf16() {
                nomes.extend_from_slice(&u.to_le_bytes());
            }
            nomes.extend_from_slice(&[0, 0]);
        }
        propriedade(v, id::NOME, &nomes);

        let datas: Vec<bool> = self.itens.iter().map(|i| i.modificado.is_some()).collect();
        if datas.iter().any(|d| *d) {
            let mut corpo = Vec::new();
            if datas.iter().all(|d| *d) {
                corpo.push(1);
            } else {
                corpo.push(0);
                escrever_bits(&mut corpo, &datas);
            }
            corpo.push(0);
            for m in self.itens.iter().filter_map(|i| i.modificado) {
                corpo.extend_from_slice(&m.to_le_bytes());
            }
            propriedade(v, id::MODIFICADO, &corpo);
        }

        let mut corpo = alloc::vec![1u8, 0];
        for item in &self.itens {
            let a = if item.pasta {
                ATRIBUTO_PASTA
            } else {
                ATRIBUTO_ARQUIVO
            };
            corpo.extend_from_slice(&a.to_le_bytes());
        }
        propriedade(v, id::ATRIBUTOS, &corpo);
        escrever_numero(v, id::FIM);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::leitor::Cursor;

    #[test]
    fn numero_vai_e_volta_em_toda_largura() {
        let mut valores = alloc::vec![0u64, 1, 0x7f, 0x80, 0x3fff, 0x4000, u64::MAX];
        for b in 0..64 {
            valores.push(1u64 << b);
            valores.push((1u64 << b) - 1);
        }
        for n in valores {
            let mut v = Vec::new();
            escrever_numero(&mut v, n);
            let mut c = Cursor::novo(&v);
            assert_eq!(c.numero().unwrap(), n, "{n:#x} -> {v:?}");
        }
    }

    #[test]
    fn identificador_do_coder_sem_zeros_a_esquerda() {
        let mut v = Vec::new();
        escrever_coder(&mut v, ID_COPIA, &[]);
        assert_eq!(v, [0x01, 0x00]);
        let mut v = Vec::new();
        escrever_coder(&mut v, ID_7ZAES, &[9]);
        assert_eq!(v, [0x24, 0x06, 0xf1, 0x07, 0x01, 0x01, 0x09]);
    }
}
