//! `.ndx` -- os indices da tabela, em B+tree, todos no mesmo arquivo.
//!
//! # Por que a arvore nao conhece tipos
//!
//! As chaves chegam aqui ja codificadas por `phxsql_core::keyenc`, que preserva
//! ordem: comparar bytes com `memcmp` da o mesmo resultado que comparar os
//! valores logicos. A B+tree entao so compara bytes, e o mesmo codigo serve
//! para inteiro, data, decimal, texto, ASC, DESC e NOCASE.
//!
//! # Chave completa
//!
//! Cada entrada de folha guarda a "chave completa" = chave do usuario seguida
//! do rowid em big-endian:
//!
//! ```text
//! [chave codificada: key_len bytes][rowid: 8 bytes BE]
//! ```
//!
//! Como o rowid entra no fim e em BE, toda chave completa e unica e a
//! comparacao byte a byte tambem desempata por rowid. Indices duplicados
//! saem de graca, e o indice unico e imposto por uma consulta de prefixo
//! antes de inserir.
//!
//! # Paginas
//!
//! ```text
//! pagina 0      cabecalho (128 bytes) + diretorio de indices
//! pagina n>0    no da arvore
//!
//! cabecalho de pagina (32 bytes):
//!   [tipo u8][flags u8][qtd u16][proxima_folha u64]
//!   [pagina_anterior u64][filho_direita u64][crc32 u32]
//!
//! folha:   entrada = chave completa            (ck_len bytes)
//! interno: entrada = chave completa + filho    (ck_len + 8 bytes)
//! ```
//!
//! Num no interno, o filho da entrada `i` guarda as chaves MENORES que a
//! chave da entrada `i`; `filho_direita` guarda as maiores ou iguais a
//! ultima chave.
//!
//! # Remocao
//!
//! Remover tira a entrada da folha sem rebalancear a arvore. A busca continua
//! correta (folhas vazias apenas nao produzem resultado), mas paginas podem
//! ficar subocupadas depois de muitas exclusoes. A reconstrucao do indice
//! (feita pela compactacao da tabela) devolve a arvore ao formato compacto.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use phxsql_core::crc::{crc32, crc32_with};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::RowId;

use crate::util::{
    agora, conferir_magic, escrever_em, ler_exato, por_i64, por_u16, por_u32, por_u64, Campos,
};

pub const MAGIC_NDX: &[u8; 8] = b"PHXNDX\0\0";
const CAB_LEN: usize = 128;
const PAG_CAB: usize = 32;
const VERSAO: u16 = 1;
pub const PAGINA_PADRAO: usize = 4096;

#[allow(dead_code)]
const TIPO_LIVRE: u8 = 0;
const TIPO_FOLHA: u8 = 1;
const TIPO_INTERNO: u8 = 2;

/// Minimo de entradas por pagina para que a divisao funcione.
const MIN_ENTRADAS: usize = 4;

/// Tamanho do rowid anexado a toda chave.
pub const ROWID_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescritorIndice {
    pub nome: String,
    pub unico: bool,
    /// Bytes da chave codificada do usuario, sem o rowid.
    pub key_len: usize,
    pub raiz: u64,
    pub qtd_chaves: u64,
}

impl DescritorIndice {
    /// Bytes da chave completa (chave do usuario + rowid).
    pub fn ck_len(&self) -> usize {
        self.key_len + ROWID_LEN
    }
}

pub struct NdxFile {
    arquivo: File,
    caminho: PathBuf,
    page_size: usize,
    qtd_paginas: u64,
    pagina_livre: u64,
    indices: Vec<DescritorIndice>,
}

// ---------------------------------------------------------------- paginas

fn pag_tipo(p: &[u8]) -> u8 {
    p[0]
}
fn pag_qtd(p: &[u8]) -> usize {
    Campos(p).u16(2) as usize
}
fn pag_set_qtd(p: &mut [u8], v: usize) {
    por_u16(p, 2, v as u16);
}
fn pag_prox(p: &[u8]) -> u64 {
    Campos(p).u64(4)
}
fn pag_set_prox(p: &mut [u8], v: u64) {
    por_u64(p, 4, v);
}
fn pag_ant(p: &[u8]) -> u64 {
    Campos(p).u64(12)
}
fn pag_set_ant(p: &mut [u8], v: u64) {
    por_u64(p, 12, v);
}
fn pag_dir(p: &[u8]) -> u64 {
    Campos(p).u64(20)
}
fn pag_set_dir(p: &mut [u8], v: u64) {
    por_u64(p, 20, v);
}

/// CRC da pagina, calculado sobre tudo menos os proprios 4 bytes do CRC.
fn pag_crc(p: &[u8]) -> u32 {
    crc32_with(crc32(&p[..28]), &p[32..])
}
fn pag_selar(p: &mut [u8]) {
    let c = pag_crc(p);
    por_u32(p, 28, c);
}

fn nova_pagina(page_size: usize, tipo: u8) -> Vec<u8> {
    let mut p = vec![0u8; page_size];
    p[0] = tipo;
    p
}

// -------------------------------------------------------------- entradas

fn folha_entrada(p: &[u8], i: usize, ck_len: usize) -> &[u8] {
    &p[PAG_CAB + i * ck_len..PAG_CAB + (i + 1) * ck_len]
}

fn interno_chave(p: &[u8], i: usize, ck_len: usize) -> &[u8] {
    let ent = ck_len + 8;
    &p[PAG_CAB + i * ent..PAG_CAB + i * ent + ck_len]
}

fn interno_filho(p: &[u8], i: usize, ck_len: usize) -> u64 {
    let ent = ck_len + 8;
    Campos(p).u64(PAG_CAB + i * ent + ck_len)
}

fn interno_set_filho(p: &mut [u8], i: usize, ck_len: usize, filho: u64) {
    let ent = ck_len + 8;
    por_u64(p, PAG_CAB + i * ent + ck_len, filho);
}

/// Primeira posicao cuja entrada e >= `alvo`.
fn lower_bound_folha(p: &[u8], ck_len: usize, alvo: &[u8]) -> usize {
    let (mut lo, mut hi) = (0usize, pag_qtd(p));
    while lo < hi {
        let mid = (lo + hi) / 2;
        if folha_entrada(p, mid, ck_len) < alvo {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Indice do filho a seguir num no interno. `qtd` significa `filho_direita`.
fn escolher_filho(p: &[u8], ck_len: usize, alvo: &[u8]) -> usize {
    let (mut lo, mut hi) = (0usize, pag_qtd(p));
    while lo < hi {
        let mid = (lo + hi) / 2;
        if alvo < interno_chave(p, mid, ck_len) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

// ------------------------------------------------------------- NdxFile

impl NdxFile {
    /// Cria o `.ndx` com uma arvore vazia para cada indice do esquema.
    pub fn criar(caminho: impl AsRef<Path>, esquema: &Schema) -> Result<NdxFile> {
        Self::criar_com_pagina(caminho, esquema, PAGINA_PADRAO)
    }

    pub fn criar_com_pagina(
        caminho: impl AsRef<Path>,
        esquema: &Schema,
        page_size: usize,
    ) -> Result<NdxFile> {
        if !page_size.is_power_of_two() || page_size < 512 {
            return Err(PhxError::Esquema(format!(
                "page_size {page_size} invalido: use potencia de 2 >= 512"
            )));
        }
        let caminho = caminho.as_ref().to_path_buf();
        let arquivo = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&caminho)?;

        let mut n = NdxFile {
            arquivo,
            caminho,
            page_size,
            qtd_paginas: 1, // pagina 0 = cabecalho + diretorio
            pagina_livre: 0,
            indices: Vec::new(),
        };
        n.arquivo.set_len(page_size as u64)?;

        for (i, idx) in esquema.indices().iter().enumerate() {
            let key_len = esquema.largura_chave(i)?;
            let ck_len = key_len + ROWID_LEN;
            n.validar_capacidade(ck_len, &idx.nome)?;
            let raiz = n.alocar_pagina()?;
            let mut folha = nova_pagina(page_size, TIPO_FOLHA);
            n.gravar_pagina(raiz, &mut folha)?;
            n.indices.push(DescritorIndice {
                nome: idx.nome.clone(),
                unico: idx.unico,
                key_len,
                raiz,
                qtd_chaves: 0,
            });
        }
        n.gravar_cabecalho()?;
        Ok(n)
    }

    pub fn abrir(caminho: impl AsRef<Path>) -> Result<NdxFile> {
        let caminho = caminho.as_ref().to_path_buf();
        let mut arquivo = OpenOptions::new().read(true).write(true).open(&caminho)?;
        let nome = caminho.display().to_string();

        let mut cab = [0u8; CAB_LEN];
        ler_exato(&mut arquivo, 0, &mut cab)?;
        conferir_magic(&nome, MAGIC_NDX, &cab[0..8])?;

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

        let page_size = c.u32(12) as usize;
        let qtd_indices = c.u32(16) as usize;
        let qtd_paginas = c.u64(20);
        let pagina_livre = c.u64(28);
        let dir_len = c.u32(36) as usize;
        let dir_crc = c.u32(40);

        let mut dir = vec![0u8; dir_len];
        ler_exato(&mut arquivo, CAB_LEN as u64, &mut dir)?;
        if crc32(&dir) != dir_crc {
            return Err(PhxError::Corrompido(format!(
                "diretorio de indices de {nome} com CRC invalido"
            )));
        }

        let mut indices = Vec::with_capacity(qtd_indices);
        let mut pos = 0usize;
        for _ in 0..qtd_indices {
            if pos + 2 > dir.len() {
                return Err(PhxError::Corrompido(format!(
                    "diretorio de indices de {nome} truncado"
                )));
            }
            let nl = u16::from_le_bytes(dir[pos..pos + 2].try_into().unwrap()) as usize;
            if pos + 2 + nl + 21 > dir.len() {
                return Err(PhxError::Corrompido(format!(
                    "diretorio de indices de {nome} truncado"
                )));
            }
            pos += 2;
            let nome_idx = String::from_utf8(dir[pos..pos + nl].to_vec())
                .map_err(|e| PhxError::Corrompido(format!("nome de indice invalido: {e}")))?;
            pos += nl;
            let unico = dir[pos] != 0;
            pos += 1;
            let key_len = u32::from_le_bytes(dir[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            let raiz = u64::from_le_bytes(dir[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let qtd_chaves = u64::from_le_bytes(dir[pos..pos + 8].try_into().unwrap());
            pos += 8;
            indices.push(DescritorIndice {
                nome: nome_idx,
                unico,
                key_len,
                raiz,
                qtd_chaves,
            });
        }

        Ok(NdxFile {
            arquivo,
            caminho,
            page_size,
            qtd_paginas,
            pagina_livre,
            indices,
        })
    }

    fn validar_capacidade(&self, ck_len: usize, nome: &str) -> Result<()> {
        let cap_folha = (self.page_size - PAG_CAB) / ck_len;
        let cap_interno = (self.page_size - PAG_CAB) / (ck_len + 8);
        if cap_folha < MIN_ENTRADAS || cap_interno < MIN_ENTRADAS {
            return Err(PhxError::Esquema(format!(
                "indice {nome}: chave de {ck_len} bytes e grande demais para paginas de {} bytes \
                 (cabem {cap_folha} por folha, minimo {MIN_ENTRADAS})",
                self.page_size
            )));
        }
        Ok(())
    }

    fn serializar_diretorio(&self) -> Vec<u8> {
        let mut d = Vec::new();
        for i in &self.indices {
            let nb = i.nome.as_bytes();
            d.extend_from_slice(&(nb.len() as u16).to_le_bytes());
            d.extend_from_slice(nb);
            d.push(i.unico as u8);
            d.extend_from_slice(&(i.key_len as u32).to_le_bytes());
            d.extend_from_slice(&i.raiz.to_le_bytes());
            d.extend_from_slice(&i.qtd_chaves.to_le_bytes());
        }
        d
    }

    fn gravar_cabecalho(&mut self) -> Result<()> {
        let dir = self.serializar_diretorio();
        if CAB_LEN + dir.len() > self.page_size {
            return Err(PhxError::LimiteExcedido(format!(
                "diretorio de {} indices nao cabe na pagina 0 de {} bytes",
                self.indices.len(),
                self.page_size
            )));
        }
        let mut buf = vec![0u8; self.page_size];
        buf[0..8].copy_from_slice(MAGIC_NDX);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, self.page_size as u32);
        por_u32(&mut buf, 16, self.indices.len() as u32);
        por_u64(&mut buf, 20, self.qtd_paginas);
        por_u64(&mut buf, 28, self.pagina_livre);
        por_u32(&mut buf, 36, dir.len() as u32);
        por_u32(&mut buf, 40, crc32(&dir));
        por_i64(&mut buf, 44, agora());
        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);
        buf[CAB_LEN..CAB_LEN + dir.len()].copy_from_slice(&dir);
        escrever_em(&mut self.arquivo, 0, &buf)
    }

    fn ler_pagina(&mut self, n: u64) -> Result<Vec<u8>> {
        if n == 0 || n >= self.qtd_paginas {
            return Err(PhxError::Corrompido(format!(
                "pagina {n} fora do arquivo {}",
                self.caminho.display()
            )));
        }
        let mut p = vec![0u8; self.page_size];
        ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
        if pag_crc(&p) != Campos(&p).u32(28) {
            return Err(PhxError::Corrompido(format!(
                "CRC invalido na pagina {n} de {}",
                self.caminho.display()
            )));
        }
        Ok(p)
    }

    fn gravar_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        pag_selar(p);
        escrever_em(&mut self.arquivo, n * self.page_size as u64, p)
    }

    fn alocar_pagina(&mut self) -> Result<u64> {
        if self.pagina_livre != 0 {
            let n = self.pagina_livre;
            let mut p = vec![0u8; self.page_size];
            ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
            self.pagina_livre = pag_prox(&p);
            return Ok(n);
        }
        let n = self.qtd_paginas;
        self.qtd_paginas += 1;
        self.arquivo
            .set_len(self.qtd_paginas * self.page_size as u64)?;
        Ok(n)
    }

    pub fn indices(&self) -> &[DescritorIndice] {
        &self.indices
    }

    pub fn indice_por_nome(&self, nome: &str) -> Option<usize> {
        self.indices.iter().position(|i| i.nome == nome)
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn paginas(&self) -> u64 {
        self.qtd_paginas
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }

    /// Monta a chave completa: chave do usuario + rowid em big-endian.
    pub fn chave_completa(chave: &[u8], rowid: RowId) -> Vec<u8> {
        let mut ck = Vec::with_capacity(chave.len() + ROWID_LEN);
        ck.extend_from_slice(chave);
        ck.extend_from_slice(&rowid.to_be_bytes());
        ck
    }

    fn descritor(&self, idx: usize) -> Result<&DescritorIndice> {
        self.indices
            .get(idx)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {idx} inexistente")))
    }

    // ------------------------------------------------------------ insercao

    /// Insere `chave` (codificada, sem rowid) apontando para `rowid`.
    /// Insere conferindo a unicidade antes.
    pub fn inserir(&mut self, idx: usize, chave: &[u8], rowid: RowId) -> Result<()> {
        if self.descritor(idx)?.unico && self.existe(idx, chave)? {
            let nome = self.descritor(idx)?.nome.clone();
            return Err(PhxError::Duplicado(format!(
                "indice unico {nome} ja tem essa chave"
            )));
        }
        self.inserir_ja_conferido(idx, chave, rowid)
    }

    /// Insere SEM conferir unicidade, para quem ja conferiu.
    ///
    /// Existe por medicao. A `Table` precisa conferir antes de gravar no
    /// `.reg` -- descobrir a duplicidade depois exigiria desfazer, e o slot
    /// desfeito ficaria morto para sempre, porque o `.reg` nao reaproveita
    /// slot. So que o `inserir` conferia de novo aqui dentro, e cada
    /// conferencia e uma descida inteira na arvore: duas descidas para
    /// responder a mesma pergunta, em toda insercao de todo indice unico.
    ///
    /// Quem chamar isto assume a conferencia. Chamar sem ter conferido mete
    /// chave repetida num indice unico, e o indice passa a mentir.
    pub fn inserir_ja_conferido(&mut self, idx: usize, chave: &[u8], rowid: RowId) -> Result<()> {
        let d = self.descritor(idx)?.clone();
        if chave.len() != d.key_len {
            return Err(PhxError::Corrompido(format!(
                "indice {}: chave de {} bytes, esperado {}",
                d.nome,
                chave.len(),
                d.key_len
            )));
        }

        let ck = Self::chave_completa(chave, rowid);
        if let Some((promovida, nova)) = self.inserir_rec(d.raiz, &ck, d.ck_len())? {
            let nova_raiz = self.alocar_pagina()?;
            let mut p = nova_pagina(self.page_size, TIPO_INTERNO);
            pag_set_qtd(&mut p, 1);
            let ck_len = d.ck_len();
            p[PAG_CAB..PAG_CAB + ck_len].copy_from_slice(&promovida);
            interno_set_filho(&mut p, 0, ck_len, d.raiz);
            pag_set_dir(&mut p, nova);
            self.gravar_pagina(nova_raiz, &mut p)?;
            self.indices[idx].raiz = nova_raiz;
        }
        self.indices[idx].qtd_chaves += 1;
        self.gravar_cabecalho()
    }

    /// Devolve `Some((chave_promovida, pagina_nova))` quando a pagina dividiu.
    fn inserir_rec(
        &mut self,
        pagina: u64,
        ck: &[u8],
        ck_len: usize,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let mut p = self.ler_pagina(pagina)?;
        match pag_tipo(&p) {
            TIPO_FOLHA => self.inserir_folha(pagina, &mut p, ck, ck_len),
            TIPO_INTERNO => {
                let pos = escolher_filho(&p, ck_len, ck);
                let filho = if pos < pag_qtd(&p) {
                    interno_filho(&p, pos, ck_len)
                } else {
                    pag_dir(&p)
                };
                match self.inserir_rec(filho, ck, ck_len)? {
                    None => Ok(None),
                    Some((promovida, nova)) => {
                        // Reler: a recursao pode ter mexido em outras paginas.
                        let mut p = self.ler_pagina(pagina)?;
                        self.inserir_interno(pagina, &mut p, pos, &promovida, nova, ck_len)
                    }
                }
            }
            outro => Err(PhxError::Corrompido(format!(
                "pagina {pagina} com tipo desconhecido {outro}"
            ))),
        }
    }

    fn inserir_folha(
        &mut self,
        pagina: u64,
        p: &mut [u8],
        ck: &[u8],
        ck_len: usize,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let qtd = pag_qtd(p);
        let pos = lower_bound_folha(p, ck_len, ck);
        if pos < qtd && folha_entrada(p, pos, ck_len) == ck {
            return Err(PhxError::Duplicado(
                "chave completa ja existe no indice".into(),
            ));
        }
        let cap = (self.page_size - PAG_CAB) / ck_len;

        if qtd < cap {
            let inicio = PAG_CAB + pos * ck_len;
            let fim_dados = PAG_CAB + qtd * ck_len;
            p.copy_within(inicio..fim_dados, inicio + ck_len);
            p[inicio..inicio + ck_len].copy_from_slice(ck);
            pag_set_qtd(p, qtd + 1);
            self.gravar_pagina(pagina, p)?;
            return Ok(None);
        }

        // Divisao: monta a lista completa e reparte.
        let mut entradas: Vec<Vec<u8>> = (0..qtd)
            .map(|i| folha_entrada(p, i, ck_len).to_vec())
            .collect();
        entradas.insert(pos, ck.to_vec());
        let meio = entradas.len() / 2;

        let nova = self.alocar_pagina()?;
        let prox_antiga = pag_prox(p);

        let mut esq = nova_pagina(self.page_size, TIPO_FOLHA);
        pag_set_qtd(&mut esq, meio);
        pag_set_ant(&mut esq, pag_ant(p));
        pag_set_prox(&mut esq, nova);
        for (i, e) in entradas[..meio].iter().enumerate() {
            esq[PAG_CAB + i * ck_len..PAG_CAB + (i + 1) * ck_len].copy_from_slice(e);
        }

        let mut dir = nova_pagina(self.page_size, TIPO_FOLHA);
        pag_set_qtd(&mut dir, entradas.len() - meio);
        pag_set_ant(&mut dir, pagina);
        pag_set_prox(&mut dir, prox_antiga);
        for (i, e) in entradas[meio..].iter().enumerate() {
            dir[PAG_CAB + i * ck_len..PAG_CAB + (i + 1) * ck_len].copy_from_slice(e);
        }

        let promovida = entradas[meio].clone();
        self.gravar_pagina(pagina, &mut esq)?;
        self.gravar_pagina(nova, &mut dir)?;

        // A folha seguinte passa a apontar para a nova como anterior.
        if prox_antiga != 0 {
            let mut seguinte = self.ler_pagina(prox_antiga)?;
            pag_set_ant(&mut seguinte, nova);
            self.gravar_pagina(prox_antiga, &mut seguinte)?;
        }

        Ok(Some((promovida, nova)))
    }

    fn inserir_interno(
        &mut self,
        pagina: u64,
        p: &mut [u8],
        pos: usize,
        promovida: &[u8],
        nova_filha: u64,
        ck_len: usize,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let qtd = pag_qtd(p);
        let ent = ck_len + 8;
        let cap = (self.page_size - PAG_CAB) / ent;

        if qtd < cap {
            if pos < qtd {
                let filho_que_dividiu = interno_filho(p, pos, ck_len);
                let inicio = PAG_CAB + pos * ent;
                let fim = PAG_CAB + qtd * ent;
                p.copy_within(inicio..fim, inicio + ent);
                p[inicio..inicio + ck_len].copy_from_slice(promovida);
                interno_set_filho(p, pos, ck_len, filho_que_dividiu);
                interno_set_filho(p, pos + 1, ck_len, nova_filha);
            } else {
                let antiga_direita = pag_dir(p);
                let inicio = PAG_CAB + qtd * ent;
                p[inicio..inicio + ck_len].copy_from_slice(promovida);
                interno_set_filho(p, qtd, ck_len, antiga_direita);
                pag_set_dir(p, nova_filha);
            }
            pag_set_qtd(p, qtd + 1);
            self.gravar_pagina(pagina, p)?;
            return Ok(None);
        }

        // Monta a lista logica (chave, filho) ja com a nova entrada.
        let mut entradas: Vec<(Vec<u8>, u64)> = (0..qtd)
            .map(|i| {
                (
                    interno_chave(p, i, ck_len).to_vec(),
                    interno_filho(p, i, ck_len),
                )
            })
            .collect();
        let mut direita = pag_dir(p);

        if pos < qtd {
            let filho_que_dividiu = entradas[pos].1;
            entradas[pos].1 = nova_filha;
            entradas.insert(pos, (promovida.to_vec(), filho_que_dividiu));
        } else {
            entradas.push((promovida.to_vec(), direita));
            direita = nova_filha;
        }

        let meio = entradas.len() / 2;
        let (chave_promovida, filho_meio) = entradas[meio].clone();

        let mut esq = nova_pagina(self.page_size, TIPO_INTERNO);
        pag_set_qtd(&mut esq, meio);
        for (i, (k, f)) in entradas[..meio].iter().enumerate() {
            esq[PAG_CAB + i * ent..PAG_CAB + i * ent + ck_len].copy_from_slice(k);
            interno_set_filho(&mut esq, i, ck_len, *f);
        }
        pag_set_dir(&mut esq, filho_meio);

        let resto = &entradas[meio + 1..];
        let mut dirp = nova_pagina(self.page_size, TIPO_INTERNO);
        pag_set_qtd(&mut dirp, resto.len());
        for (i, (k, f)) in resto.iter().enumerate() {
            dirp[PAG_CAB + i * ent..PAG_CAB + i * ent + ck_len].copy_from_slice(k);
            interno_set_filho(&mut dirp, i, ck_len, *f);
        }
        pag_set_dir(&mut dirp, direita);

        let nova = self.alocar_pagina()?;
        self.gravar_pagina(pagina, &mut esq)?;
        self.gravar_pagina(nova, &mut dirp)?;
        Ok(Some((chave_promovida, nova)))
    }

    // -------------------------------------------------------------- busca

    /// Desce ate a folha que deve conter `alvo` e devolve (pagina, posicao).
    /// Desce ate a folha onde a chave entraria.
    ///
    /// Devolve a folha JUNTO com o numero e a posicao. Antes ela era lida aqui
    /// e jogada fora, e quem chamava lia de novo -- uma pagina inteira a mais
    /// por busca, com o CRC junto.
    fn descer(&mut self, raiz: u64, alvo: &[u8], ck_len: usize) -> Result<(u64, usize, Vec<u8>)> {
        let mut pagina = raiz;
        loop {
            let p = self.ler_pagina(pagina)?;
            match pag_tipo(&p) {
                TIPO_FOLHA => {
                    let pos = lower_bound_folha(&p, ck_len, alvo);
                    return Ok((pagina, pos, p));
                }
                TIPO_INTERNO => {
                    let pos = escolher_filho(&p, ck_len, alvo);
                    pagina = if pos < pag_qtd(&p) {
                        interno_filho(&p, pos, ck_len)
                    } else {
                        pag_dir(&p)
                    };
                }
                outro => {
                    return Err(PhxError::Corrompido(format!(
                        "pagina {pagina} com tipo desconhecido {outro}"
                    )))
                }
            }
        }
    }

    /// Ha ao menos uma entrada com esta chave?
    ///
    /// E o que a conferencia de unicidade precisa saber, e so isso. O `buscar`
    /// junta TODOS os rowids num vetor para depois alguem perguntar se o vetor
    /// esta vazio -- num indice unico a resposta cabe numa comparacao, e num
    /// indice comum juntar mil rowids para descartar os mil e trabalho jogado
    /// fora.
    pub fn existe(&mut self, idx: usize, chave: &[u8]) -> Result<bool> {
        let d = self.descritor(idx)?.clone();
        if chave.len() != d.key_len {
            return Err(PhxError::Corrompido(format!(
                "indice {}: chave de {} bytes, esperado {}",
                d.nome,
                chave.len(),
                d.key_len
            )));
        }
        let ck_len = d.ck_len();
        let inicio = Self::chave_completa(chave, 0);
        let (_pagina, pos, folha) = self.descer(d.raiz, &inicio, ck_len)?;

        // A chave pode cair exatamente no fim de uma folha: a primeira entrada
        // com esse prefixo estaria na folha seguinte.
        if pos < pag_qtd(&folha) {
            return Ok(folha_entrada(&folha, pos, ck_len)[..d.key_len] == *chave);
        }
        let proxima = pag_prox(&folha);
        if proxima == 0 {
            return Ok(false);
        }
        let p = self.ler_pagina(proxima)?;
        if pag_qtd(&p) == 0 {
            return Ok(false);
        }
        Ok(folha_entrada(&p, 0, ck_len)[..d.key_len] == *chave)
    }

    /// Todos os rowids cuja chave e exatamente `chave`.
    /// Como o rowid entra no fim da chave completa, o resultado sai ordenado
    /// por rowid, ou seja, na ordem de digitacao.
    pub fn buscar(&mut self, idx: usize, chave: &[u8]) -> Result<Vec<RowId>> {
        let d = self.descritor(idx)?.clone();
        if chave.len() != d.key_len {
            return Err(PhxError::Corrompido(format!(
                "indice {}: chave de {} bytes, esperado {}",
                d.nome,
                chave.len(),
                d.key_len
            )));
        }
        let inicio = Self::chave_completa(chave, 0);
        self.coletar(&d, &inicio, |e| &e[..d.key_len] == chave)
    }

    /// Rowids no intervalo `[de, ate]` (ambos opcionais, `ate` inclusivo).
    pub fn intervalo(
        &mut self,
        idx: usize,
        de: Option<&[u8]>,
        ate: Option<&[u8]>,
    ) -> Result<Vec<RowId>> {
        let d = self.descritor(idx)?.clone();
        let inicio = match de {
            Some(k) => Self::chave_completa(k, 0),
            None => vec![0u8; d.ck_len()],
        };
        let ate = ate.map(|k| k.to_vec());
        self.coletar(&d, &inicio, move |e| match &ate {
            Some(limite) => &e[..limite.len()] <= limite.as_slice(),
            None => true,
        })
    }

    /// Todos os rowids do indice, na ordem do indice.
    pub fn varrer(&mut self, idx: usize) -> Result<Vec<RowId>> {
        self.intervalo(idx, None, None)
    }

    fn coletar<F>(
        &mut self,
        d: &DescritorIndice,
        inicio: &[u8],
        mut aceita: F,
    ) -> Result<Vec<RowId>>
    where
        F: FnMut(&[u8]) -> bool,
    {
        let ck_len = d.ck_len();
        let (mut pagina, mut pos, mut folha) = self.descer(d.raiz, inicio, ck_len)?;
        let mut saida = Vec::new();
        while pagina != 0 {
            // A primeira folha vem da descida; as seguintes se leem aqui.
            let p = folha;
            let qtd = pag_qtd(&p);
            while pos < qtd {
                let e = folha_entrada(&p, pos, ck_len);
                if !aceita(e) {
                    return Ok(saida);
                }
                let rowid = u64::from_be_bytes(e[d.key_len..].try_into().unwrap());
                saida.push(rowid);
                pos += 1;
            }
            pagina = pag_prox(&p);
            pos = 0;
            if pagina == 0 {
                break;
            }
            folha = self.ler_pagina(pagina)?;
        }
        Ok(saida)
    }

    // ------------------------------------------------------------ remocao

    /// Remove a entrada (`chave`, `rowid`). Devolve `false` se nao existia.
    pub fn remover(&mut self, idx: usize, chave: &[u8], rowid: RowId) -> Result<bool> {
        let d = self.descritor(idx)?.clone();
        let ck_len = d.ck_len();
        let ck = Self::chave_completa(chave, rowid);
        let (pagina, pos, mut p) = self.descer(d.raiz, &ck, ck_len)?;
        let qtd = pag_qtd(&p);
        if pos >= qtd || folha_entrada(&p, pos, ck_len) != ck.as_slice() {
            return Ok(false);
        }
        let inicio = PAG_CAB + pos * ck_len;
        let fim = PAG_CAB + qtd * ck_len;
        p.copy_within(inicio + ck_len..fim, inicio);
        p[fim - ck_len..fim].fill(0);
        pag_set_qtd(&mut p, qtd - 1);
        self.gravar_pagina(pagina, &mut p)?;
        self.indices[idx].qtd_chaves = self.indices[idx].qtd_chaves.saturating_sub(1);
        self.gravar_cabecalho()?;
        Ok(true)
    }

    // --------------------------------------------------------- verificacao

    /// Confere CRC de todas as paginas e a ordenacao das folhas de cada
    /// indice. Devolve a quantidade de chaves encontrada por indice.
    pub fn verificar(&mut self) -> Result<Vec<(String, u64)>> {
        let mut saida = Vec::new();
        for i in 0..self.indices.len() {
            let d = self.indices[i].clone();
            let ck_len = d.ck_len();
            let inicio = vec![0u8; ck_len];
            let (mut pagina, _, _) = self.descer(d.raiz, &inicio, ck_len)?;
            let mut anterior: Option<Vec<u8>> = None;
            let mut total = 0u64;
            while pagina != 0 {
                let p = self.ler_pagina(pagina)?; // ja confere CRC
                for j in 0..pag_qtd(&p) {
                    let e = folha_entrada(&p, j, ck_len);
                    if let Some(ant) = &anterior {
                        if e <= ant.as_slice() {
                            return Err(PhxError::Corrompido(format!(
                                "indice {} fora de ordem na pagina {pagina}",
                                d.nome
                            )));
                        }
                    }
                    anterior = Some(e.to_vec());
                    total += 1;
                }
                pagina = pag_prox(&p);
            }
            if total != d.qtd_chaves {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: diretorio diz {} chaves, varredura achou {total}",
                    d.nome, d.qtd_chaves
                )));
            }
            saida.push((d.nome, total));
        }
        Ok(saida)
    }
}
