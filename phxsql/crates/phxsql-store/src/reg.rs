//! `.reg` -- a tabela fisica, na ordem de digitacao.
//!
//! O `.reg` e um heap de slots de largura fixa. O rowid e o numero do slot,
//! comecando em 1, e o endereco sai de uma conta, nao de uma busca:
//!
//! ```text
//! offset(rowid) = data_offset + (rowid - 1) * slot_size
//! ```
//!
//! # Ordem de digitacao
//!
//! Registros sao SEMPRE anexados no fim. Excluir marca o slot como livre, mas
//! o slot nao e reaproveitado: isso manteria o arquivo compacto ao custo de
//! quebrar a garantia de que percorrer o `.reg` do inicio ao fim devolve os
//! registros na ordem em que foram digitados. O espaco de slots excluidos so
//! volta com uma compactacao explicita, que renumera os rowids e reconstroi
//! os indices.
//!
//! # Layout
//!
//! ```text
//! cabecalho     128 bytes
//! esquema       schema_len bytes (serializado, auto-descritivo)
//! [alinhamento ate multiplo de 64]
//! slot 1, slot 2, ...
//!
//! slot: [status u8][flags u8][res u16][crc32 payload u32]
//!       [versao u64][res u64][payload ...]
//! ```

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::RowId;

use crate::util::{
    agora, conferir_magic, escrever_em, ler_exato, por_i64, por_u32, por_u64, Campos,
};

pub const MAGIC_REG: &[u8; 8] = b"PHXREG\0\0";
const CAB_LEN: usize = 128;
/// Bytes de cabecalho de cada slot, antes do payload.
pub const SLOT_CAB: usize = 24;
const VERSAO: u16 = 1;
const ALINHAMENTO: u64 = 64;

const STATUS_LIVRE: u8 = 0;
const STATUS_ATIVO: u8 = 1;

pub struct RegFile {
    arquivo: File,
    caminho: PathBuf,
    esquema: Schema,
    slot_size: usize,
    data_offset: u64,
    slot_count: u64,
    live_count: u64,
    criado_em: i64,
}

impl RegFile {
    pub fn criar(caminho: impl AsRef<Path>, esquema: Schema) -> Result<RegFile> {
        let caminho = caminho.as_ref().to_path_buf();
        let arquivo = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&caminho)?;

        let bytes_esquema = esquema.serializar();
        let data_offset = alinhar(CAB_LEN as u64 + bytes_esquema.len() as u64, ALINHAMENTO);
        let slot_size = SLOT_CAB + esquema.payload_len();

        let mut r = RegFile {
            arquivo,
            caminho,
            esquema,
            slot_size,
            data_offset,
            slot_count: 0,
            live_count: 0,
            criado_em: agora(),
        };
        escrever_em(&mut r.arquivo, CAB_LEN as u64, &bytes_esquema)?;
        r.gravar_cabecalho()?;
        // Garante que o arquivo tenha ao menos ate data_offset.
        r.arquivo.set_len(data_offset)?;
        Ok(r)
    }

    pub fn abrir(caminho: impl AsRef<Path>) -> Result<RegFile> {
        let caminho = caminho.as_ref().to_path_buf();
        let mut arquivo = OpenOptions::new().read(true).write(true).open(&caminho)?;
        let nome = caminho.display().to_string();

        let mut cab = [0u8; CAB_LEN];
        ler_exato(&mut arquivo, 0, &mut cab)?;
        conferir_magic(&nome, MAGIC_REG, &cab[0..8])?;

        let c = Campos(&cab);
        let versao = c.u16(8);
        if versao != VERSAO {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: nome,
                encontrada: versao,
                suportada: VERSAO,
            });
        }
        if crc32(&cab[..124]) != c.u32(124) {
            return Err(PhxError::Corrompido(format!(
                "cabecalho de {nome} com CRC invalido"
            )));
        }

        let slot_size = c.u32(16) as usize;
        let slot_count = c.u64(20);
        let live_count = c.u64(28);
        let data_offset = c.u64(44);
        let schema_len = c.u32(52) as usize;
        let schema_crc = c.u32(56);
        let criado_em = c.u64(60) as i64;

        let mut bytes_esquema = vec![0u8; schema_len];
        ler_exato(&mut arquivo, CAB_LEN as u64, &mut bytes_esquema)?;
        if crc32(&bytes_esquema) != schema_crc {
            return Err(PhxError::Corrompido(format!(
                "esquema de {nome} com CRC invalido"
            )));
        }
        let esquema = Schema::desserializar(&bytes_esquema)?;

        let esperado = SLOT_CAB + esquema.payload_len();
        if slot_size != esperado {
            return Err(PhxError::Corrompido(format!(
                "slot_size {slot_size} em {nome} nao bate com o esquema ({esperado})"
            )));
        }

        Ok(RegFile {
            arquivo,
            caminho,
            esquema,
            slot_size,
            data_offset,
            slot_count,
            live_count,
            criado_em,
        })
    }

    fn gravar_cabecalho(&mut self) -> Result<()> {
        let bytes_esquema = self.esquema.serializar();
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(MAGIC_REG);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 16, self.slot_size as u32);
        por_u64(&mut buf, 20, self.slot_count);
        por_u64(&mut buf, 28, self.live_count);
        por_u64(&mut buf, 44, self.data_offset);
        por_u32(&mut buf, 52, bytes_esquema.len() as u32);
        por_u32(&mut buf, 56, crc32(&bytes_esquema));
        por_i64(&mut buf, 60, self.criado_em);
        por_i64(&mut buf, 68, agora());
        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);
        escrever_em(&mut self.arquivo, 0, &buf)
    }

    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Total de slots ja alocados, incluindo os excluidos.
    /// Tambem e o maior rowid ja atribuido.
    pub fn slots(&self) -> u64 {
        self.slot_count
    }

    /// Registros ativos.
    pub fn registros(&self) -> u64 {
        self.live_count
    }

    pub fn slot_size(&self) -> usize {
        self.slot_size
    }

    fn offset(&self, rowid: RowId) -> u64 {
        self.data_offset + (rowid - 1) * self.slot_size as u64
    }

    fn conferir_faixa(&self, rowid: RowId) -> Result<()> {
        if rowid == 0 || rowid > self.slot_count {
            return Err(PhxError::NaoEncontrado(format!(
                "rowid {rowid} fora da faixa 1..={} em {}",
                self.slot_count,
                self.caminho.display()
            )));
        }
        Ok(())
    }

    /// Anexa um registro no fim e devolve seu rowid.
    pub fn inserir(&mut self, payload: &[u8]) -> Result<RowId> {
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let rowid = self.slot_count + 1;
        let offset = self.offset(rowid);

        let mut slot = vec![0u8; self.slot_size];
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, 1); // versao do registro
        slot[SLOT_CAB..].copy_from_slice(payload);

        escrever_em(&mut self.arquivo, offset, &slot)?;
        self.slot_count += 1;
        self.live_count += 1;
        self.gravar_cabecalho()?;
        Ok(rowid)
    }

    /// Le o payload de um registro. Devolve `None` se o slot foi excluido.
    pub fn ler(&mut self, rowid: RowId) -> Result<Option<Vec<u8>>> {
        self.conferir_faixa(rowid)?;
        let offset = self.offset(rowid);
        let mut slot = vec![0u8; self.slot_size];
        ler_exato(&mut self.arquivo, offset, &mut slot)?;
        if slot[0] != STATUS_ATIVO {
            return Ok(None);
        }
        let payload = slot[SLOT_CAB..].to_vec();
        let gravado = Campos(&slot).u32(4);
        if crc32(&payload) != gravado {
            return Err(PhxError::Corrompido(format!(
                "CRC do registro {rowid} em {} nao confere",
                self.caminho.display()
            )));
        }
        Ok(Some(payload))
    }

    pub fn ativo(&mut self, rowid: RowId) -> Result<bool> {
        self.conferir_faixa(rowid)?;
        let offset = self.offset(rowid);
        let mut b = [0u8; 1];
        ler_exato(&mut self.arquivo, offset, &mut b)?;
        Ok(b[0] == STATUS_ATIVO)
    }

    /// Regrava o payload de um registro existente, no mesmo slot.
    /// O rowid e a posicao fisica nao mudam.
    pub fn atualizar(&mut self, rowid: RowId, payload: &[u8]) -> Result<()> {
        self.conferir_faixa(rowid)?;
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let offset = self.offset(rowid);
        let mut slot = vec![0u8; self.slot_size];
        ler_exato(&mut self.arquivo, offset, &mut slot)?;
        if slot[0] != STATUS_ATIVO {
            return Err(PhxError::NaoEncontrado(format!(
                "registro {rowid} esta excluido"
            )));
        }
        let versao = Campos(&slot).u64(8);
        slot[..SLOT_CAB].fill(0);
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, versao.saturating_add(1));
        slot[SLOT_CAB..].copy_from_slice(payload);
        escrever_em(&mut self.arquivo, offset, &slot)?;
        self.gravar_cabecalho()
    }

    /// Marca o registro como excluido. Devolve `false` se ja estava excluido.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        self.conferir_faixa(rowid)?;
        let offset = self.offset(rowid);
        let mut cab = [0u8; SLOT_CAB];
        ler_exato(&mut self.arquivo, offset, &mut cab)?;
        if cab[0] != STATUS_ATIVO {
            return Ok(false);
        }
        cab[0] = STATUS_LIVRE;
        escrever_em(&mut self.arquivo, offset, &cab)?;
        self.live_count = self.live_count.saturating_sub(1);
        self.gravar_cabecalho()?;
        Ok(true)
    }

    /// Proximo registro ativo com rowid >= `desde`, na ordem de digitacao.
    pub fn proximo_ativo(&mut self, desde: RowId) -> Result<Option<(RowId, Vec<u8>)>> {
        let mut rowid = desde.max(1);
        while rowid <= self.slot_count {
            if let Some(p) = self.ler(rowid)? {
                return Ok(Some((rowid, p)));
            }
            rowid += 1;
        }
        Ok(None)
    }

    /// Confere o CRC de todos os registros ativos e a contagem do cabecalho.
    pub fn verificar(&mut self) -> Result<u64> {
        let mut vivos = 0u64;
        for rowid in 1..=self.slot_count {
            if self.ler(rowid)?.is_some() {
                vivos += 1;
            }
        }
        if vivos != self.live_count {
            return Err(PhxError::Corrompido(format!(
                "{}: cabecalho diz {} registros, varredura achou {vivos}",
                self.caminho.display(),
                self.live_count
            )));
        }
        Ok(vivos)
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }
}

fn alinhar(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn temp(nome: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-reg-{}-{}", std::process::id(), nome));
        p
    }

    fn esquema() -> Schema {
        Schema::new(
            "cadastroClientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(30)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    fn payload(esq: &Schema, n: u8) -> Vec<u8> {
        let mut p = vec![0u8; esq.payload_len()];
        p[esq.bitmap_len()] = n;
        p
    }

    #[test]
    fn insere_le_e_conta() {
        let caminho = temp("insere.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        let mut r = RegFile::criar(&caminho, esq.clone()).unwrap();

        assert_eq!(r.inserir(&payload(&esq, 10)).unwrap(), 1);
        assert_eq!(r.inserir(&payload(&esq, 20)).unwrap(), 2);
        assert_eq!(r.inserir(&payload(&esq, 30)).unwrap(), 3);
        assert_eq!(r.slots(), 3);
        assert_eq!(r.registros(), 3);
        assert_eq!(r.ler(2).unwrap().unwrap(), payload(&esq, 20));
        assert_eq!(r.verificar().unwrap(), 3);
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn exclusao_nao_reaproveita_slot_e_preserva_a_ordem() {
        let caminho = temp("ordem.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        let mut r = RegFile::criar(&caminho, esq.clone()).unwrap();
        for n in 1..=5u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        assert!(r.excluir(3).unwrap());
        assert!(!r.excluir(3).unwrap());
        assert_eq!(r.registros(), 4);
        assert_eq!(r.slots(), 5);

        // O proximo insert vai para o slot 6, nao para o buraco do slot 3.
        assert_eq!(r.inserir(&payload(&esq, 6)).unwrap(), 6);
        assert!(r.ler(3).unwrap().is_none());

        // Varredura devolve a ordem de digitacao, pulando o excluido.
        let mut vistos = Vec::new();
        let mut rowid = 1;
        while let Some((id, p)) = r.proximo_ativo(rowid).unwrap() {
            vistos.push((id, p[esq.bitmap_len()]));
            rowid = id + 1;
        }
        assert_eq!(vistos, vec![(1, 1), (2, 2), (4, 4), (5, 5), (6, 6)]);
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn atualiza_no_mesmo_slot() {
        let caminho = temp("update.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        let mut r = RegFile::criar(&caminho, esq.clone()).unwrap();
        let id = r.inserir(&payload(&esq, 1)).unwrap();
        r.atualizar(id, &payload(&esq, 99)).unwrap();
        assert_eq!(r.ler(id).unwrap().unwrap()[esq.bitmap_len()], 99);
        assert_eq!(r.slots(), 1);
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn atualizar_excluido_e_erro() {
        let caminho = temp("upd-excl.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        let mut r = RegFile::criar(&caminho, esq.clone()).unwrap();
        let id = r.inserir(&payload(&esq, 1)).unwrap();
        r.excluir(id).unwrap();
        assert!(r.atualizar(id, &payload(&esq, 2)).is_err());
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn reabre_com_esquema_auto_descritivo() {
        let caminho = temp("reabre.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        {
            let mut r = RegFile::criar(&caminho, esq.clone()).unwrap();
            r.inserir(&payload(&esq, 7)).unwrap();
            r.sincronizar().unwrap();
        }
        let mut r = RegFile::abrir(&caminho).unwrap();
        assert_eq!(r.esquema(), &esq);
        assert_eq!(r.esquema().nome(), "cadastroClientes");
        assert_eq!(r.ler(1).unwrap().unwrap()[esq.bitmap_len()], 7);
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn rowid_fora_da_faixa_e_erro() {
        let caminho = temp("faixa.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        let mut r = RegFile::criar(&caminho, esq).unwrap();
        assert!(r.ler(0).is_err());
        assert!(r.ler(1).is_err());
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn registro_adulterado_falha_no_crc() {
        let caminho = temp("crc.reg");
        let _ = std::fs::remove_file(&caminho);
        let esq = esquema();
        let offset = {
            let mut r = RegFile::criar(&caminho, esq.clone()).unwrap();
            r.inserir(&payload(&esq, 5)).unwrap();
            r.sincronizar().unwrap();
            r.offset(1)
        };
        {
            let mut f = OpenOptions::new().write(true).open(&caminho).unwrap();
            escrever_em(&mut f, offset + SLOT_CAB as u64 + 1, b"\xFF").unwrap();
        }
        let mut r = RegFile::abrir(&caminho).unwrap();
        assert!(r.ler(1).is_err());
        std::fs::remove_file(&caminho).unwrap();
    }
}
