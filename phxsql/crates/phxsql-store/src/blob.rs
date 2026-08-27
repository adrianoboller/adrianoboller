//! Arquivos externos `.bin` (binarios) e `.memo` (textos longos).
//!
//! Os dois usam a mesma estrutura -- uma pilha de blocos append-only -- e
//! diferem apenas na assinatura e na semantica do conteudo: o `.memo` guarda
//! UTF-8 e o `.bin` guarda bytes crus.
//!
//! ```text
//! cabecalho (64 bytes)
//! bloco: [status u8][res 3][tamanho u32][crc32 u32][res 4][conteudo ...]
//! ```
//!
//! Atualizar um conteudo NAO reescreve o bloco antigo: grava um bloco novo no
//! fim e marca o antigo como morto. O espaco morto volta com a compactacao.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::value::Ponteiro;

use crate::util::{
    agora, conferir_magic, escrever_em, ler_exato, por_i64, por_u32, por_u64, Campos,
};

pub const MAGIC_BIN: &[u8; 8] = b"PHXBIN\0\0";
pub const MAGIC_MEMO: &[u8; 8] = b"PHXMEMO\0";

const CAB_LEN: usize = 64;
pub(crate) const BLOCO_CAB: usize = 16;
const VERSAO: u16 = 1;

#[derive(Debug, Clone, Copy)]
struct Cabecalho {
    fim: u64,
    bytes_vivos: u64,
    bytes_mortos: u64,
    qtd_blocos: u64,
}

/// Estatisticas de ocupacao de um arquivo externo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstatisticaBlob {
    pub blocos: u64,
    pub bytes_vivos: u64,
    pub bytes_mortos: u64,
    pub tamanho_arquivo: u64,
}

impl EstatisticaBlob {
    /// Fracao de espaco morto, de 0.0 a 1.0. Serve para decidir compactacao.
    pub fn fragmentacao(&self) -> f64 {
        let total = self.bytes_vivos + self.bytes_mortos;
        if total == 0 {
            0.0
        } else {
            self.bytes_mortos as f64 / total as f64
        }
    }
}

pub struct BlobFile {
    arquivo: File,
    caminho: PathBuf,
    magic: &'static [u8; 8],
    cab: Cabecalho,
}

impl BlobFile {
    pub fn criar(caminho: impl AsRef<Path>, magic: &'static [u8; 8]) -> Result<BlobFile> {
        let caminho = caminho.as_ref().to_path_buf();
        let arquivo = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&caminho)?;

        let mut b = BlobFile {
            arquivo,
            caminho,
            magic,
            cab: Cabecalho {
                fim: CAB_LEN as u64,
                bytes_vivos: 0,
                bytes_mortos: 0,
                qtd_blocos: 0,
            },
        };
        b.gravar_cabecalho()?;
        Ok(b)
    }

    pub fn abrir(caminho: impl AsRef<Path>, magic: &'static [u8; 8]) -> Result<BlobFile> {
        let caminho = caminho.as_ref().to_path_buf();
        let mut arquivo = OpenOptions::new().read(true).write(true).open(&caminho)?;

        let mut buf = [0u8; CAB_LEN];
        ler_exato(&mut arquivo, 0, &mut buf)?;
        let nome = caminho.display().to_string();
        conferir_magic(&nome, magic, &buf[0..8])?;

        let c = Campos(&buf);
        let versao = c.u16(8);
        if versao != VERSAO {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: nome,
                encontrada: versao,
                suportada: VERSAO,
            });
        }
        if crc32(&buf[..56]) != c.u32(56) {
            return Err(PhxError::Corrompido(format!(
                "cabecalho de {nome} com CRC invalido"
            )));
        }

        let cab = Cabecalho {
            fim: c.u64(16),
            bytes_vivos: c.u64(24),
            bytes_mortos: c.u64(32),
            qtd_blocos: c.u64(40),
        };

        Ok(BlobFile {
            arquivo,
            caminho,
            magic,
            cab,
        })
    }

    fn gravar_cabecalho(&mut self) -> Result<()> {
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(self.magic);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u64(&mut buf, 16, self.cab.fim);
        por_u64(&mut buf, 24, self.cab.bytes_vivos);
        por_u64(&mut buf, 32, self.cab.bytes_mortos);
        por_u64(&mut buf, 40, self.cab.qtd_blocos);
        por_i64(&mut buf, 48, agora());
        let crc = crc32(&buf[..56]);
        por_u32(&mut buf, 56, crc);
        escrever_em(&mut self.arquivo, 0, &buf)
    }

    /// Grava um conteudo novo no fim do arquivo e devolve o ponteiro.
    ///
    /// Conteudo vazio nao consome bloco: devolve [`Ponteiro::VAZIO`].
    pub fn gravar(&mut self, dados: &[u8]) -> Result<Ponteiro> {
        if dados.is_empty() {
            return Ok(Ponteiro::VAZIO);
        }
        let tamanho = u32::try_from(dados.len()).map_err(|_| {
            PhxError::LimiteExcedido(format!(
                "bloco de {} bytes excede o maximo de 4 GiB",
                dados.len()
            ))
        })?;
        let crc = crc32(dados);
        let offset = self.cab.fim;

        let mut cab = [0u8; BLOCO_CAB];
        cab[0] = 1; // vivo
        por_u32(&mut cab, 4, tamanho);
        por_u32(&mut cab, 8, crc);

        self.arquivo.seek(SeekFrom::Start(offset))?;
        self.arquivo.write_all(&cab)?;
        self.arquivo.write_all(dados)?;

        self.cab.fim = offset + BLOCO_CAB as u64 + tamanho as u64;
        self.cab.bytes_vivos += tamanho as u64;
        self.cab.qtd_blocos += 1;
        self.gravar_cabecalho()?;

        Ok(Ponteiro {
            offset,
            tamanho,
            crc,
        })
    }

    /// Le o conteudo apontado, conferindo o CRC gravado.
    pub fn ler(&mut self, p: &Ponteiro) -> Result<Vec<u8>> {
        if p.e_vazio() {
            return Ok(Vec::new());
        }
        if p.offset < CAB_LEN as u64 || p.offset + BLOCO_CAB as u64 > self.cab.fim {
            return Err(PhxError::Corrompido(format!(
                "ponteiro fora do arquivo {}: offset {}",
                self.caminho.display(),
                p.offset
            )));
        }

        let mut cab = [0u8; BLOCO_CAB];
        ler_exato(&mut self.arquivo, p.offset, &mut cab)?;
        let c = Campos(&cab);
        let tamanho = c.u32(4);
        if tamanho != p.tamanho {
            return Err(PhxError::Corrompido(format!(
                "tamanho divergente em {}: ponteiro diz {}, bloco diz {tamanho}",
                self.caminho.display(),
                p.tamanho
            )));
        }

        let mut dados = vec![0u8; tamanho as usize];
        ler_exato(&mut self.arquivo, p.offset + BLOCO_CAB as u64, &mut dados)?;
        let real = crc32(&dados);
        if real != p.crc || real != c.u32(8) {
            return Err(PhxError::Corrompido(format!(
                "CRC do bloco em {} offset {} nao confere",
                self.caminho.display(),
                p.offset
            )));
        }
        Ok(dados)
    }

    /// Marca o bloco como morto. O espaco so volta com a compactacao.
    pub fn liberar(&mut self, p: &Ponteiro) -> Result<()> {
        if p.e_vazio() {
            return Ok(());
        }
        let mut cab = [0u8; BLOCO_CAB];
        ler_exato(&mut self.arquivo, p.offset, &mut cab)?;
        if cab[0] == 0 {
            return Ok(()); // ja estava morto
        }
        cab[0] = 0;
        escrever_em(&mut self.arquivo, p.offset, &cab)?;

        self.cab.bytes_vivos = self.cab.bytes_vivos.saturating_sub(p.tamanho as u64);
        self.cab.bytes_mortos += p.tamanho as u64;
        self.gravar_cabecalho()
    }

    pub fn estatistica(&self) -> EstatisticaBlob {
        EstatisticaBlob {
            blocos: self.cab.qtd_blocos,
            bytes_vivos: self.cab.bytes_vivos,
            bytes_mortos: self.cab.bytes_mortos,
            tamanho_arquivo: self.cab.fim,
        }
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }

    /// Percorre todos os blocos e confere o CRC de cada um dos vivos.
    /// Devolve (blocos vivos conferidos, blocos mortos).
    pub fn verificar(&mut self) -> Result<(u64, u64)> {
        let mut vivos = 0u64;
        let mut mortos = 0u64;
        let mut offset = CAB_LEN as u64;
        while offset + BLOCO_CAB as u64 <= self.cab.fim {
            let mut cab = [0u8; BLOCO_CAB];
            ler_exato(&mut self.arquivo, offset, &mut cab)?;
            let c = Campos(&cab);
            let tamanho = c.u32(4);
            if offset + BLOCO_CAB as u64 + tamanho as u64 > self.cab.fim {
                return Err(PhxError::Corrompido(format!(
                    "bloco em {} offset {offset} ultrapassa o fim do arquivo",
                    self.caminho.display()
                )));
            }
            if cab[0] == 1 {
                let mut dados = vec![0u8; tamanho as usize];
                ler_exato(&mut self.arquivo, offset + BLOCO_CAB as u64, &mut dados)?;
                if crc32(&dados) != c.u32(8) {
                    return Err(PhxError::Corrompido(format!(
                        "CRC invalido em {} offset {offset}",
                        self.caminho.display()
                    )));
                }
                vivos += 1;
            } else {
                mortos += 1;
            }
            offset += BLOCO_CAB as u64 + tamanho as u64;
        }
        Ok((vivos, mortos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(nome: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-blob-{}-{}", std::process::id(), nome));
        p
    }

    #[test]
    fn grava_le_e_libera() {
        let caminho = temp("basico.memo");
        let _ = std::fs::remove_file(&caminho);
        let mut b = BlobFile::criar(&caminho, MAGIC_MEMO).unwrap();

        let texto = "observacao bem longa do cliente".repeat(50);
        let p = b.gravar(texto.as_bytes()).unwrap();
        assert_eq!(b.ler(&p).unwrap(), texto.as_bytes());

        let est = b.estatistica();
        assert_eq!(est.blocos, 1);
        assert_eq!(est.bytes_vivos, texto.len() as u64);
        assert_eq!(est.bytes_mortos, 0);

        b.liberar(&p).unwrap();
        let est = b.estatistica();
        assert_eq!(est.bytes_vivos, 0);
        assert_eq!(est.bytes_mortos, texto.len() as u64);
        assert!((est.fragmentacao() - 1.0).abs() < f64::EPSILON);

        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn reabre_preservando_estado() {
        let caminho = temp("reabre.bin");
        let _ = std::fs::remove_file(&caminho);
        let p = {
            let mut b = BlobFile::criar(&caminho, MAGIC_BIN).unwrap();
            let p = b.gravar(&[1u8, 2, 3, 4, 5]).unwrap();
            b.sincronizar().unwrap();
            p
        };
        let mut b = BlobFile::abrir(&caminho, MAGIC_BIN).unwrap();
        assert_eq!(b.ler(&p).unwrap(), vec![1, 2, 3, 4, 5]);
        assert_eq!(b.estatistica().blocos, 1);
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn magic_errado_e_recusado() {
        let caminho = temp("magic.bin");
        let _ = std::fs::remove_file(&caminho);
        {
            let mut b = BlobFile::criar(&caminho, MAGIC_BIN).unwrap();
            b.sincronizar().unwrap();
        }
        assert!(BlobFile::abrir(&caminho, MAGIC_MEMO).is_err());
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn conteudo_adulterado_falha_no_crc() {
        let caminho = temp("crc.bin");
        let _ = std::fs::remove_file(&caminho);
        let p = {
            let mut b = BlobFile::criar(&caminho, MAGIC_BIN).unwrap();
            let p = b.gravar(b"conteudo integro").unwrap();
            b.sincronizar().unwrap();
            p
        };
        {
            let mut f = OpenOptions::new().write(true).open(&caminho).unwrap();
            escrever_em(&mut f, p.offset + BLOCO_CAB as u64 + 2, b"X").unwrap();
        }
        let mut b = BlobFile::abrir(&caminho, MAGIC_BIN).unwrap();
        assert!(b.ler(&p).is_err());
        assert!(b.verificar().is_err());
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn vazio_nao_consome_bloco() {
        let caminho = temp("vazio.memo");
        let _ = std::fs::remove_file(&caminho);
        let mut b = BlobFile::criar(&caminho, MAGIC_MEMO).unwrap();
        let p = b.gravar(b"").unwrap();
        assert!(p.e_vazio());
        assert_eq!(b.estatistica().blocos, 0);
        assert!(b.ler(&p).unwrap().is_empty());
        std::fs::remove_file(&caminho).unwrap();
    }
}
