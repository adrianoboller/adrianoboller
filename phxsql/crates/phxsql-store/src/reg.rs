//! `.reg` -- a tabela fisica, na ordem de digitacao.
//!
//! O `.reg` e um heap de slots de largura fixa. O rowid e o numero do slot
//! dentro da TABELA (nao dentro do volume), comecando em 1, e o endereco sai
//! de uma conta, nao de uma busca:
//!
//! ```text
//! volume = (rowid - 1) / registros_por_arquivo + 1
//! slot   = (rowid - 1) % registros_por_arquivo + 1
//! offset = data_offset + (slot - 1) * slot_size
//! ```
//!
//! Sem paginacao o volume e sempre 1 e o slot e o proprio rowid.
//!
//! # Ordem de digitacao
//!
//! Registros sao SEMPRE anexados no fim. Excluir marca o slot como livre, mas
//! o slot nao e reaproveitado: isso manteria o arquivo compacto ao custo de
//! quebrar a garantia de que percorrer o `.reg` do inicio ao fim devolve os
//! registros na ordem em que foram digitados. O espaco de slots excluidos so
//! volta com uma compactacao explicita.
//!
//! Com paginacao a garantia continua valendo: o volume N+1 vem sempre depois
//! do N, e dentro de cada volume os slots seguem em ordem de insercao.
//!
//! # Layout de cada volume
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
//!
//! Todo volume carrega o cabecalho completo com o esquema, entao qualquer um
//! deles se descreve sozinho. Apenas o volume 1 tem contadores autoritativos
//! da tabela inteira.

use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::Schema;
use phxsql_core::{RowId, EXT_REG};

use crate::util::{agora, conferir_magic, por_i64, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_REG: &[u8; 8] = b"PHXREG\0\0";
const CAB_LEN: usize = 128;
/// Bytes de cabecalho de cada slot, antes do payload.
pub const SLOT_CAB: usize = 24;
const VERSAO: u16 = 2;
const ALINHAMENTO: u64 = 64;

const STATUS_LIVRE: u8 = 0;
const STATUS_ATIVO: u8 = 1;

pub struct RegFile {
    volumes: Volumes,
    esquema: Schema,
    slot_size: usize,
    data_offset: u64,
    slot_count: u64,
    live_count: u64,
    criado_em: i64,
}

impl RegFile {
    pub fn criar(diretorio: impl AsRef<Path>, nome: &str, esquema: Schema) -> Result<RegFile> {
        let paginacao = esquema.paginacao();
        let bytes_esquema = esquema.serializar();
        let data_offset = alinhar(CAB_LEN as u64 + bytes_esquema.len() as u64, ALINHAMENTO);
        let slot_size = SLOT_CAB + esquema.payload_len();

        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, paginacao),
            esquema,
            slot_size,
            data_offset,
            slot_count: 0,
            live_count: 0,
            criado_em: agora(),
        };
        r.volumes.criar(1)?;
        r.gravar_cabecalho(1)?;
        Ok(r)
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<RegFile> {
        // A paginacao mora dentro do esquema, que mora dentro do primeiro
        // volume -- e a largura do sufixo faz parte dela. Para nao chutar,
        // acha-se o primeiro volume varrendo o diretorio e le-se o cabecalho
        // direto, antes de montar o conjunto de volumes.
        let primeiro = achar_primeiro_volume(diretorio.as_ref(), nome, EXT_REG)?;
        let nome_arq = primeiro.display().to_string();
        let bruto = std::fs::read(&primeiro)?;
        if bruto.len() < CAB_LEN {
            return Err(PhxError::Corrompido(format!("{nome_arq} truncado")));
        }
        let mut cab = [0u8; CAB_LEN];
        cab.copy_from_slice(&bruto[..CAB_LEN]);
        conferir_magic(&nome_arq, MAGIC_REG, &cab[0..8])?;

        let c = Campos(&cab);
        let versao = c.u16(8);
        if versao != VERSAO {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: nome_arq,
                encontrada: versao,
                suportada: VERSAO,
            });
        }
        if crc32(&cab[..124]) != c.u32(124) {
            return Err(PhxError::Corrompido(format!(
                "cabecalho de {nome_arq} com CRC invalido"
            )));
        }

        let slot_size = c.u32(16) as usize;
        let slot_count = c.u64(20);
        let live_count = c.u64(28);
        let data_offset = c.u64(44);
        let schema_len = c.u32(52) as usize;
        let schema_crc = c.u32(56);
        let criado_em = c.u64(60) as i64;

        if bruto.len() < CAB_LEN + schema_len {
            return Err(PhxError::Corrompido(format!(
                "{nome_arq} nao contem o esquema inteiro"
            )));
        }
        let bytes_esquema = bruto[CAB_LEN..CAB_LEN + schema_len].to_vec();
        if crc32(&bytes_esquema) != schema_crc {
            return Err(PhxError::Corrompido(format!(
                "esquema de {nome_arq} com CRC invalido"
            )));
        }
        let esquema = Schema::desserializar(&bytes_esquema)?;

        let esperado = SLOT_CAB + esquema.payload_len();
        if slot_size != esperado {
            return Err(PhxError::Corrompido(format!(
                "slot_size {slot_size} em {nome_arq} nao bate com o esquema ({esperado})"
            )));
        }

        Ok(RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, esquema.paginacao()),
            esquema,
            slot_size,
            data_offset,
            slot_count,
            live_count,
            criado_em,
        })
    }

    fn gravar_cabecalho(&mut self, volume: u32) -> Result<()> {
        let bytes_esquema = self.esquema.serializar();
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(MAGIC_REG);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, volume);
        por_u32(&mut buf, 16, self.slot_size as u32);
        // Contadores da tabela inteira: so o volume 1 e autoritativo.
        if volume == 1 {
            por_u64(&mut buf, 20, self.slot_count);
            por_u64(&mut buf, 28, self.live_count);
        }
        por_u64(&mut buf, 44, self.data_offset);
        por_u32(&mut buf, 52, bytes_esquema.len() as u32);
        por_u32(&mut buf, 56, crc32(&bytes_esquema));
        por_i64(&mut buf, 60, self.criado_em);
        por_i64(&mut buf, 68, agora());
        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);

        self.volumes.escrever(volume, 0, &buf)?;
        self.volumes
            .escrever(volume, CAB_LEN as u64, &bytes_esquema)?;
        if self.volumes.tamanho(volume)? < self.data_offset {
            self.volumes.definir_tamanho(volume, self.data_offset)?;
        }
        Ok(())
    }

    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }

    pub fn caminho(&self, volume: u32) -> PathBuf {
        self.volumes.caminho(volume)
    }

    pub fn volumes(&self) -> Vec<u32> {
        self.volumes.existentes()
    }

    pub fn paginacao(&self) -> Paginacao {
        self.esquema.paginacao()
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

    /// Volume e offset em que um rowid mora.
    fn localizar(&self, rowid: RowId) -> (u32, u64) {
        let (volume, slot) = self.esquema.paginacao().localizar(rowid);
        (
            volume,
            self.data_offset + (slot - 1) * self.slot_size as u64,
        )
    }

    fn conferir_faixa(&self, rowid: RowId) -> Result<()> {
        if rowid == 0 || rowid > self.slot_count {
            return Err(PhxError::NaoEncontrado(format!(
                "rowid {rowid} fora da faixa 1..={} em {}",
                self.slot_count,
                self.volumes.nome()
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
        let paginacao = self.esquema.paginacao();
        if !paginacao.cabe(rowid) {
            return Err(PhxError::LimiteExcedido(format!(
                "tabela {} cheia: capacidade de {} registros ({} por arquivo x {} arquivos)",
                self.volumes.nome(),
                paginacao.capacidade(),
                paginacao.registros_por_arquivo,
                paginacao.max_arquivos
            )));
        }

        let (volume, offset) = self.localizar(rowid);
        if self.volumes.garantir(volume)? {
            // Volume novo: ganha cabecalho e esquema proprios.
            self.gravar_cabecalho(volume)?;
        }

        let mut slot = vec![0u8; self.slot_size];
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, 1); // versao do registro
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;

        self.slot_count += 1;
        self.live_count += 1;
        self.gravar_cabecalho(1)?;
        Ok(rowid)
    }

    /// Le o payload de um registro. Devolve `None` se o slot foi excluido.
    pub fn ler(&mut self, rowid: RowId) -> Result<Option<Vec<u8>>> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut slot = vec![0u8; self.slot_size];
        self.volumes.ler(volume, offset, &mut slot)?;
        if slot[0] != STATUS_ATIVO {
            return Ok(None);
        }
        let payload = slot[SLOT_CAB..].to_vec();
        if crc32(&payload) != Campos(&slot).u32(4) {
            return Err(PhxError::Corrompido(format!(
                "CRC do registro {rowid} em {} nao confere",
                self.volumes.caminho(volume).display()
            )));
        }
        Ok(Some(payload))
    }

    pub fn ativo(&mut self, rowid: RowId) -> Result<bool> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut b = [0u8; 1];
        self.volumes.ler(volume, offset, &mut b)?;
        Ok(b[0] == STATUS_ATIVO)
    }

    /// Regrava o payload de um registro existente, no mesmo slot.
    /// O rowid e a posicao fisica nao mudam.
    /// Devolve a nova versao do registro.
    pub fn atualizar(&mut self, rowid: RowId, payload: &[u8]) -> Result<u64> {
        self.conferir_faixa(rowid)?;
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let (volume, offset) = self.localizar(rowid);
        let mut slot = vec![0u8; self.slot_size];
        self.volumes.ler(volume, offset, &mut slot)?;
        if slot[0] != STATUS_ATIVO {
            return Err(PhxError::NaoEncontrado(format!(
                "registro {rowid} esta excluido"
            )));
        }
        let versao = Campos(&slot).u64(8).saturating_add(1);
        slot[..SLOT_CAB].fill(0);
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, versao);
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;
        self.gravar_cabecalho(1)?;
        Ok(versao)
    }

    /// Marca o registro como excluido. Devolve `false` se ja estava excluido.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut cab = [0u8; SLOT_CAB];
        self.volumes.ler(volume, offset, &mut cab)?;
        if cab[0] != STATUS_ATIVO {
            return Ok(false);
        }
        cab[0] = STATUS_LIVRE;
        self.volumes.escrever(volume, offset, &cab)?;
        self.live_count = self.live_count.saturating_sub(1);
        self.gravar_cabecalho(1)?;
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
                self.volumes.nome(),
                self.live_count
            )));
        }
        Ok(vivos)
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.volumes.sincronizar()
    }
}

fn alinhar(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// Acha o primeiro volume de um conjunto sem saber, de antemao, se a tabela e
/// paginada nem qual a largura do sufixo.
///
/// Procura primeiro `nome.ext` (tabela em arquivo unico); se nao existir,
/// varre o diretorio atras de `nome_<digitos>.ext` e devolve o menor.
fn achar_primeiro_volume(diretorio: &Path, nome: &str, ext: &str) -> Result<PathBuf> {
    let simples = diretorio.join(format!("{nome}.{ext}"));
    if simples.exists() {
        return Ok(simples);
    }
    let prefixo = format!("{nome}_");
    let mut candidatos: Vec<PathBuf> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if p.extension().and_then(|s| s.to_str()) != Some(ext) {
                return false;
            }
            match p.file_stem().and_then(|s| s.to_str()) {
                Some(base) => match base.strip_prefix(&prefixo) {
                    Some(sufixo) => {
                        !sufixo.is_empty() && sufixo.chars().all(|c| c.is_ascii_digit())
                    }
                    None => false,
                },
                None => false,
            }
        })
        .collect();
    candidatos.sort();
    candidatos.into_iter().next().ok_or_else(|| {
        PhxError::NaoEncontrado(format!(
            "nenhum volume de {nome}.{ext} em {}",
            diretorio.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-reg-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
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
        let d = dir_temp("insere");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        assert_eq!(r.inserir(&payload(&esq, 10)).unwrap(), 1);
        assert_eq!(r.inserir(&payload(&esq, 20)).unwrap(), 2);
        assert_eq!(r.inserir(&payload(&esq, 30)).unwrap(), 3);
        assert_eq!(r.slots(), 3);
        assert_eq!(r.registros(), 3);
        assert_eq!(r.ler(2).unwrap().unwrap(), payload(&esq, 20));
        assert_eq!(r.verificar().unwrap(), 3);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn exclusao_nao_reaproveita_slot_e_preserva_a_ordem() {
        let d = dir_temp("ordem");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=5u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        assert!(r.excluir(3).unwrap());
        assert!(!r.excluir(3).unwrap());
        assert_eq!(r.registros(), 4);
        assert_eq!(r.slots(), 5);
        assert_eq!(r.inserir(&payload(&esq, 6)).unwrap(), 6);
        assert!(r.ler(3).unwrap().is_none());

        let mut vistos = Vec::new();
        let mut rowid = 1;
        while let Some((id, p)) = r.proximo_ativo(rowid).unwrap() {
            vistos.push((id, p[esq.bitmap_len()]));
            rowid = id + 1;
        }
        assert_eq!(vistos, vec![(1, 1), (2, 2), (4, 4), (5, 5), (6, 6)]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn atualiza_no_mesmo_slot() {
        let d = dir_temp("update");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        let id = r.inserir(&payload(&esq, 1)).unwrap();
        assert_eq!(r.atualizar(id, &payload(&esq, 99)).unwrap(), 2);
        assert_eq!(r.ler(id).unwrap().unwrap()[esq.bitmap_len()], 99);
        assert_eq!(r.slots(), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn atualizar_excluido_e_erro() {
        let d = dir_temp("upd-excl");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        let id = r.inserir(&payload(&esq, 1)).unwrap();
        r.excluir(id).unwrap();
        assert!(r.atualizar(id, &payload(&esq, 2)).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn reabre_com_esquema_auto_descritivo() {
        let d = dir_temp("reabre");
        let esq = esquema();
        {
            let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
            r.inserir(&payload(&esq, 7)).unwrap();
            r.sincronizar().unwrap();
        }
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert_eq!(r.esquema(), &esq);
        assert_eq!(r.esquema().nome(), "cadastroClientes");
        assert_eq!(r.ler(1).unwrap().unwrap()[esq.bitmap_len()], 7);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rowid_fora_da_faixa_e_erro() {
        let d = dir_temp("faixa");
        let mut r = RegFile::criar(&d, "cadastroClientes", esquema()).unwrap();
        assert!(r.ler(0).is_err());
        assert!(r.ler(1).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn registro_adulterado_falha_no_crc() {
        let d = dir_temp("crc");
        let esq = esquema();
        let offset = {
            let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
            r.inserir(&payload(&esq, 5)).unwrap();
            r.sincronizar().unwrap();
            r.localizar(1).1
        };
        {
            let mut v = Volumes::novo(&d, "cadastroClientes", "reg", Paginacao::DESLIGADA);
            v.escrever(1, offset + SLOT_CAB as u64 + 1, b"\xFF")
                .unwrap();
        }
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert!(r.ler(1).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    // ------------------------------------------------------------ paginacao

    fn esquema_paginado(registros: u64, arquivos: u32) -> Schema {
        esquema().com_paginacao(Paginacao::nova(registros, arquivos).unwrap())
    }

    #[test]
    fn paginacao_distribui_em_volumes_numerados() {
        let d = dir_temp("pag");
        let esq = esquema_paginado(10, 99);
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 0..25u32 {
            r.inserir(&payload(&esq, (n % 250) as u8)).unwrap();
        }
        assert_eq!(r.slots(), 25);
        assert_eq!(r.volumes(), vec![1, 2, 3]);
        assert!(d.join("cadastroClientes_001.reg").exists());
        assert!(d.join("cadastroClientes_002.reg").exists());
        assert!(d.join("cadastroClientes_003.reg").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn paginado_le_escreve_e_reabre_igual() {
        let d = dir_temp("pag-rw");
        let esq = esquema_paginado(10, 99);
        {
            let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
            for n in 1..=100u8 {
                r.inserir(&payload(&esq, n)).unwrap();
            }
            r.sincronizar().unwrap();
        }
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert_eq!(r.slots(), 100);
        assert_eq!(r.paginacao().registros_por_arquivo, 10);
        // Cada rowid volta com o conteudo certo, atravessando 10 volumes.
        for n in 1..=100u64 {
            assert_eq!(
                r.ler(n).unwrap().unwrap()[esq.bitmap_len()],
                n as u8,
                "rowid {n}"
            );
        }
        assert_eq!(r.verificar().unwrap(), 100);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn paginado_preserva_a_ordem_de_digitacao_entre_volumes() {
        let d = dir_temp("pag-ordem");
        let esq = esquema_paginado(4, 99);
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=20u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        r.excluir(7).unwrap();
        r.excluir(13).unwrap();

        let mut vistos = Vec::new();
        let mut rowid = 1;
        while let Some((id, p)) = r.proximo_ativo(rowid).unwrap() {
            vistos.push(p[esq.bitmap_len()]);
            rowid = id + 1;
        }
        let esperado: Vec<u8> = (1..=20u8).filter(|n| *n != 7 && *n != 13).collect();
        assert_eq!(vistos, esperado);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn tabela_cheia_para_de_aceitar() {
        let d = dir_temp("cheia");
        let esq = esquema_paginado(3, 2); // capacidade 6
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=6u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        let e = r.inserir(&payload(&esq, 7)).unwrap_err();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "erro foi {e}");
        assert_eq!(r.registros(), 6);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn todo_volume_se_descreve_sozinho() {
        let d = dir_temp("autodesc");
        let esq = esquema_paginado(5, 99);
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=12u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        r.sincronizar().unwrap();

        // O volume 3 carrega assinatura, versao e esquema proprios.
        let mut v = Volumes::novo(&d, "cadastroClientes", "reg", esq.paginacao());
        let mut cab = [0u8; CAB_LEN];
        v.ler(3, 0, &mut cab).unwrap();
        assert_eq!(&cab[0..8], MAGIC_REG);
        let c = Campos(&cab);
        assert_eq!(c.u16(8), VERSAO);
        assert_eq!(c.u32(12), 3, "o volume sabe o proprio numero");
        let schema_len = c.u32(52) as usize;
        let mut bytes = vec![0u8; schema_len];
        v.ler(3, CAB_LEN as u64, &mut bytes).unwrap();
        assert_eq!(Schema::desserializar(&bytes).unwrap(), esq);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
