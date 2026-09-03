//! Arquivos externos `.bin` (binarios) e `.memo` (textos longos).
//!
//! Os dois usam a mesma estrutura -- uma pilha de blocos append-only -- e
//! diferem apenas na assinatura e na semantica do conteudo: o `.memo` guarda
//! UTF-8 e o `.bin` guarda bytes crus.
//!
//! ```text
//! cabecalho do volume (64 bytes)
//! bloco: [status u8][res 3][tamanho u32][crc32 u32][res 4][conteudo ...]
//! ```
//!
//! Atualizar um conteudo NAO reescreve o bloco antigo: grava um bloco novo no
//! fim e marca o antigo como morto. O espaco morto volta com a compactacao.
//!
//! # Paginacao
//!
//! Fotos e anexos sao o que mais faz um arquivo crescer, entao o `.bin` e o
//! `.memo` tambem se partem em volumes: `Tabela_001.bin`, `Tabela_002.bin`...
//! Cada volume tem cabecalho e contabilidade proprios, e um bloco nunca e
//! partido entre dois volumes -- se nao couber no atual, vai inteiro para o
//! proximo. O `Ponteiro` gravado no `.reg` carrega o numero do volume.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::value::Ponteiro;

use crate::util::{agora, conferir_magic, por_i64, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_BIN: &[u8; 8] = b"PHXBIN\0\0";
pub const MAGIC_MEMO: &[u8; 8] = b"PHXMEMO\0";

const CAB_LEN: usize = 64;
pub(crate) const BLOCO_CAB: usize = 16;
const VERSAO: u16 = 2;

#[derive(Debug, Clone, Copy, Default)]
struct Cabecalho {
    volume: u32,
    fim: u64,
    bytes_vivos: u64,
    bytes_mortos: u64,
    qtd_blocos: u64,
}

/// Estatisticas de ocupacao, somadas sobre todos os volumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstatisticaBlob {
    pub volumes: u32,
    pub blocos: u64,
    pub bytes_vivos: u64,
    pub bytes_mortos: u64,
    pub tamanho_total: u64,
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
    volumes: Volumes,
    magic: &'static [u8; 8],
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
}

impl BlobFile {
    pub fn criar(
        diretorio: impl AsRef<Path>,
        nome: &str,
        ext: &'static str,
        magic: &'static [u8; 8],
        paginacao: Paginacao,
    ) -> Result<BlobFile> {
        let mut b = BlobFile {
            volumes: Volumes::novo(diretorio, nome, ext, paginacao),
            magic,
            cabs: HashMap::new(),
            volume_atual: 1,
        };
        b.volumes.criar(1)?;
        let cab = Cabecalho {
            volume: 1,
            fim: CAB_LEN as u64,
            ..Default::default()
        };
        b.gravar_cab(cab)?;
        Ok(b)
    }

    pub fn abrir(
        diretorio: impl AsRef<Path>,
        nome: &str,
        ext: &'static str,
        magic: &'static [u8; 8],
        paginacao: Paginacao,
    ) -> Result<BlobFile> {
        let volumes = Volumes::novo(diretorio, nome, ext, paginacao);
        let existentes = volumes.existentes();
        if existentes.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhum volume de {}",
                volumes.caminho(1).display()
            )));
        }
        let volume_atual = *existentes.last().unwrap();
        let mut b = BlobFile {
            volumes,
            magic,
            cabs: HashMap::new(),
            volume_atual,
        };
        // Conferir o volume 1 valida assinatura e versao de imediato.
        b.cab(1)?;
        b.cab(volume_atual)?;
        Ok(b)
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let mut buf = [0u8; CAB_LEN];
        self.volumes.ler(volume, 0, &mut buf)?;
        let nome = self.volumes.caminho(volume).display().to_string();
        conferir_magic(&nome, self.magic, &buf[0..8])?;

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
            volume: c.u32(12),
            fim: c.u64(16),
            bytes_vivos: c.u64(24),
            bytes_mortos: c.u64(32),
            qtd_blocos: c.u64(40),
        };
        if cab.volume != volume {
            return Err(PhxError::Corrompido(format!(
                "{nome} diz ser o volume {} mas esta no lugar do {volume}",
                cab.volume
            )));
        }
        self.cabs.insert(volume, cab);
        Ok(cab)
    }

    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(self.magic);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, cab.volume);
        por_u64(&mut buf, 16, cab.fim);
        por_u64(&mut buf, 24, cab.bytes_vivos);
        por_u64(&mut buf, 32, cab.bytes_mortos);
        por_u64(&mut buf, 40, cab.qtd_blocos);
        por_i64(&mut buf, 48, agora());
        let crc = crc32(&buf[..56]);
        por_u32(&mut buf, 56, crc);
        self.volumes.escrever(cab.volume, 0, &buf)?;
        self.cabs.insert(cab.volume, cab);
        Ok(())
    }

    /// Grava um conteudo novo e devolve o ponteiro, ja com o volume.
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

        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let precisa = BLOCO_CAB as u64 + tamanho as u64;
        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, precisa, vazio);

        let cab = if virou {
            if paginacao.ligada() && volume > paginacao.max_arquivos {
                return Err(PhxError::LimiteExcedido(format!(
                    "{} chegou ao teto de {} volumes",
                    self.volumes.nome(),
                    paginacao.max_arquivos
                )));
            }
            self.volumes.garantir(volume)?;
            let novo = Cabecalho {
                volume,
                fim: CAB_LEN as u64,
                ..Default::default()
            };
            self.gravar_cab(novo)?;
            self.volume_atual = volume;
            novo
        } else {
            atual
        };

        let crc = crc32(dados);
        let offset = cab.fim;
        let mut bloco = [0u8; BLOCO_CAB];
        bloco[0] = 1; // vivo
        por_u32(&mut bloco, 4, tamanho);
        por_u32(&mut bloco, 8, crc);
        self.volumes.escrever_par(volume, offset, &bloco, dados)?;

        self.gravar_cab(Cabecalho {
            volume,
            fim: offset + precisa,
            bytes_vivos: cab.bytes_vivos + tamanho as u64,
            bytes_mortos: cab.bytes_mortos,
            qtd_blocos: cab.qtd_blocos + 1,
        })?;

        Ok(Ponteiro {
            volume: volume as u16,
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
        let volume = p.volume.max(1) as u32;
        let cab = self.cab(volume)?;
        if p.offset < CAB_LEN as u64 || p.offset + BLOCO_CAB as u64 > cab.fim {
            return Err(PhxError::Corrompido(format!(
                "ponteiro fora do volume {}: offset {}",
                self.volumes.caminho(volume).display(),
                p.offset
            )));
        }

        let mut bloco = [0u8; BLOCO_CAB];
        self.volumes.ler(volume, p.offset, &mut bloco)?;
        let c = Campos(&bloco);
        let tamanho = c.u32(4);
        if tamanho != p.tamanho {
            return Err(PhxError::Corrompido(format!(
                "tamanho divergente em {}: ponteiro diz {}, bloco diz {tamanho}",
                self.volumes.caminho(volume).display(),
                p.tamanho
            )));
        }

        let mut dados = vec![0u8; tamanho as usize];
        self.volumes
            .ler(volume, p.offset + BLOCO_CAB as u64, &mut dados)?;
        let real = crc32(&dados);
        if real != p.crc || real != c.u32(8) {
            return Err(PhxError::Corrompido(format!(
                "CRC do bloco em {} offset {} nao confere",
                self.volumes.caminho(volume).display(),
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
        let volume = p.volume.max(1) as u32;
        let cab = self.cab(volume)?;
        let mut bloco = [0u8; BLOCO_CAB];
        self.volumes.ler(volume, p.offset, &mut bloco)?;
        if bloco[0] == 0 {
            return Ok(()); // ja estava morto
        }
        bloco[0] = 0;
        self.volumes.escrever(volume, p.offset, &bloco)?;

        self.gravar_cab(Cabecalho {
            bytes_vivos: cab.bytes_vivos.saturating_sub(p.tamanho as u64),
            bytes_mortos: cab.bytes_mortos + p.tamanho as u64,
            ..cab
        })
    }

    pub fn estatistica(&mut self) -> Result<EstatisticaBlob> {
        let mut e = EstatisticaBlob {
            volumes: 0,
            blocos: 0,
            bytes_vivos: 0,
            bytes_mortos: 0,
            tamanho_total: 0,
        };
        for v in self.volumes.existentes() {
            let c = self.cab(v)?;
            e.volumes += 1;
            e.blocos += c.qtd_blocos;
            e.bytes_vivos += c.bytes_vivos;
            e.bytes_mortos += c.bytes_mortos;
            e.tamanho_total += c.fim;
        }
        Ok(e)
    }

    pub fn caminho(&self, volume: u32) -> PathBuf {
        self.volumes.caminho(volume)
    }

    pub fn volumes(&self) -> Vec<u32> {
        self.volumes.existentes()
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.volumes.sincronizar()
    }

    /// Percorre todos os blocos de todos os volumes e confere o CRC de cada
    /// um dos vivos. Devolve (blocos vivos, blocos mortos).
    pub fn verificar(&mut self) -> Result<(u64, u64)> {
        let mut vivos = 0u64;
        let mut mortos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            let mut offset = CAB_LEN as u64;
            while offset + BLOCO_CAB as u64 <= cab.fim {
                let mut bloco = [0u8; BLOCO_CAB];
                self.volumes.ler(volume, offset, &mut bloco)?;
                let c = Campos(&bloco);
                let tamanho = c.u32(4);
                if offset + BLOCO_CAB as u64 + tamanho as u64 > cab.fim {
                    return Err(PhxError::Corrompido(format!(
                        "bloco em {} offset {offset} ultrapassa o fim do volume",
                        self.volumes.caminho(volume).display()
                    )));
                }
                if bloco[0] == 1 {
                    let mut dados = vec![0u8; tamanho as usize];
                    self.volumes
                        .ler(volume, offset + BLOCO_CAB as u64, &mut dados)?;
                    if crc32(&dados) != c.u32(8) {
                        return Err(PhxError::Corrompido(format!(
                            "CRC invalido em {} offset {offset}",
                            self.volumes.caminho(volume).display()
                        )));
                    }
                    vivos += 1;
                } else {
                    mortos += 1;
                }
                offset += BLOCO_CAB as u64 + tamanho as u64;
            }
        }
        Ok((vivos, mortos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn dir_temp(rotulo: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("blob-{rotulo}"))
    }

    #[test]
    fn grava_le_e_libera() {
        let d = dir_temp("basico");
        let mut b = BlobFile::criar(&d, "t", "memo", MAGIC_MEMO, Paginacao::DESLIGADA).unwrap();

        let texto = "observacao bem longa do cliente".repeat(50);
        let p = b.gravar(texto.as_bytes()).unwrap();
        assert_eq!(p.volume, 1);
        assert_eq!(b.ler(&p).unwrap(), texto.as_bytes());

        let e = b.estatistica().unwrap();
        assert_eq!(e.blocos, 1);
        assert_eq!(e.bytes_vivos, texto.len() as u64);

        b.liberar(&p).unwrap();
        let e = b.estatistica().unwrap();
        assert_eq!(e.bytes_vivos, 0);
        assert_eq!(e.bytes_mortos, texto.len() as u64);
        assert!((e.fragmentacao() - 1.0).abs() < f64::EPSILON);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn reabre_preservando_estado() {
        let d = dir_temp("reabre");
        let p = {
            let mut b = BlobFile::criar(&d, "t", "bin", MAGIC_BIN, Paginacao::DESLIGADA).unwrap();
            let p = b.gravar(&[1u8, 2, 3, 4, 5]).unwrap();
            b.sincronizar().unwrap();
            p
        };
        let mut b = BlobFile::abrir(&d, "t", "bin", MAGIC_BIN, Paginacao::DESLIGADA).unwrap();
        assert_eq!(b.ler(&p).unwrap(), vec![1, 2, 3, 4, 5]);
        assert_eq!(b.estatistica().unwrap().blocos, 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn magic_errado_e_recusado() {
        let d = dir_temp("magic");
        {
            let mut b = BlobFile::criar(&d, "t", "bin", MAGIC_BIN, Paginacao::DESLIGADA).unwrap();
            b.sincronizar().unwrap();
        }
        assert!(BlobFile::abrir(&d, "t", "bin", MAGIC_MEMO, Paginacao::DESLIGADA).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn conteudo_adulterado_falha_no_crc() {
        let d = dir_temp("crc");
        let p = {
            let mut b = BlobFile::criar(&d, "t", "bin", MAGIC_BIN, Paginacao::DESLIGADA).unwrap();
            let p = b.gravar(b"conteudo integro").unwrap();
            b.sincronizar().unwrap();
            p
        };
        {
            let mut v = Volumes::novo(&d, "t", "bin", Paginacao::DESLIGADA);
            v.escrever(1, p.offset + BLOCO_CAB as u64 + 2, b"X")
                .unwrap();
        }
        let mut b = BlobFile::abrir(&d, "t", "bin", MAGIC_BIN, Paginacao::DESLIGADA).unwrap();
        assert!(b.ler(&p).is_err());
        assert!(b.verificar().is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn vazio_nao_consome_bloco() {
        let d = dir_temp("vazio");
        let mut b = BlobFile::criar(&d, "t", "memo", MAGIC_MEMO, Paginacao::DESLIGADA).unwrap();
        let p = b.gravar(b"").unwrap();
        assert!(p.e_vazio());
        assert_eq!(b.estatistica().unwrap().blocos, 0);
        assert!(b.ler(&p).unwrap().is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rola_para_o_proximo_volume_sem_partir_bloco() {
        let d = dir_temp("rola");
        // Volumes de 1 KiB: cada bloco de 300 bytes ocupa 316 com o cabecalho.
        let pag = Paginacao::nova(1_000, 99)
            .unwrap()
            .com_bytes_por_arquivo(1_024)
            .unwrap();
        let mut b = BlobFile::criar(&d, "t", "bin", MAGIC_BIN, pag).unwrap();

        let mut ponteiros = Vec::new();
        for i in 0..10u8 {
            let dados = vec![i; 300];
            ponteiros.push((b.gravar(&dados).unwrap(), dados));
        }

        // Cabecalho 64 + 3 blocos de 316 = 1012; o quarto nao cabe em 1024.
        assert!(b.volumes().len() > 1, "deveria ter passado de volume");
        assert!(ponteiros.iter().any(|(p, _)| p.volume > 1));

        // Todo bloco continua legivel, no volume certo.
        for (p, esperado) in &ponteiros {
            assert_eq!(&b.ler(p).unwrap(), esperado, "volume {}", p.volume);
        }

        let e = b.estatistica().unwrap();
        assert_eq!(e.blocos, 10);
        assert_eq!(e.bytes_vivos, 3_000);
        b.verificar().unwrap();
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn teto_de_volumes_e_respeitado() {
        let d = dir_temp("teto");
        let pag = Paginacao::nova(10, 2)
            .unwrap()
            .com_bytes_por_arquivo(128)
            .unwrap();
        let mut b = BlobFile::criar(&d, "t", "bin", MAGIC_BIN, pag).unwrap();
        // Cada bloco de 100 bytes ocupa 116; cabe um por volume (64 + 116 > 128
        // ja no segundo).
        b.gravar(&[1u8; 100]).unwrap();
        b.gravar(&[2u8; 100]).unwrap();
        let e = b.gravar(&[3u8; 100]).unwrap_err();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "erro foi {e}");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
