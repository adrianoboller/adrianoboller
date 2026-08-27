//! `Table` -- a tabela de dados, que e a soma dos quatro arquivos.
//!
//! ```text
//! cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  =  cadastroClientes
//! ```
//!
//! Esta camada e quem traduz `Value` para bytes, decide o que vai inline no
//! `.reg` e o que vai para os arquivos externos, e mantem os indices em dia a
//! cada insercao, alteracao e exclusao.

use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::Schema;
use phxsql_core::types::ColumnType;
use phxsql_core::value::{escrever_inline, ler_inline, Ponteiro, Value};
use phxsql_core::{RowId, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};

use crate::blob::{BlobFile, MAGIC_BIN, MAGIC_MEMO};
use crate::ndx::NdxFile;
use crate::reg::RegFile;

/// Uma linha: um valor por coluna do esquema.
pub type Linha = Vec<Value>;

/// Resultado de uma verificacao de integridade da tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relatorio {
    pub tabela: String,
    pub registros: u64,
    pub slots: u64,
    pub indices: Vec<(String, u64)>,
    pub blocos_bin: (u64, u64),
    pub blocos_memo: (u64, u64),
}

pub struct Table {
    nome: String,
    diretorio: PathBuf,
    /// Copia do esquema que mora no `.reg`. Fica aqui para nao ser clonada a
    /// cada linha lida ou gravada.
    esquema: Schema,
    reg: RegFile,
    ndx: NdxFile,
    bin: BlobFile,
    memo: BlobFile,
}

fn caminho(diretorio: &Path, nome: &str, ext: &str) -> PathBuf {
    diretorio.join(format!("{nome}.{ext}"))
}

impl Table {
    /// Cria as quatro pecas da tabela em `diretorio`.
    ///
    /// Falha se qualquer um dos quatro arquivos ja existir, para nunca
    /// sobrescrever dados por engano.
    pub fn criar(diretorio: impl AsRef<Path>, esquema: Schema) -> Result<Table> {
        let diretorio = diretorio.as_ref().to_path_buf();
        std::fs::create_dir_all(&diretorio)?;
        let nome = esquema.nome().to_string();

        for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO] {
            let c = caminho(&diretorio, &nome, ext);
            if c.exists() {
                return Err(PhxError::Esquema(format!(
                    "{} ja existe; use Table::abrir",
                    c.display()
                )));
            }
        }

        let ndx = NdxFile::criar(caminho(&diretorio, &nome, EXT_NDX), &esquema)?;
        let bin = BlobFile::criar(caminho(&diretorio, &nome, EXT_BIN), MAGIC_BIN)?;
        let memo = BlobFile::criar(caminho(&diretorio, &nome, EXT_MEMO), MAGIC_MEMO)?;
        let reg = RegFile::criar(caminho(&diretorio, &nome, EXT_REG), esquema.clone())?;

        Ok(Table {
            nome,
            diretorio,
            esquema,
            reg,
            ndx,
            bin,
            memo,
        })
    }

    /// Abre uma tabela existente. O esquema vem de dentro do proprio `.reg`.
    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {
        let diretorio = diretorio.as_ref().to_path_buf();
        let reg = RegFile::abrir(caminho(&diretorio, nome, EXT_REG))?;
        let ndx = NdxFile::abrir(caminho(&diretorio, nome, EXT_NDX))?;
        let bin = BlobFile::abrir(caminho(&diretorio, nome, EXT_BIN), MAGIC_BIN)?;
        let memo = BlobFile::abrir(caminho(&diretorio, nome, EXT_MEMO), MAGIC_MEMO)?;

        if ndx.indices().len() != reg.esquema().indices().len() {
            return Err(PhxError::Corrompido(format!(
                "{nome}: .ndx tem {} indices, o esquema do .reg declara {}",
                ndx.indices().len(),
                reg.esquema().indices().len()
            )));
        }

        let esquema = reg.esquema().clone();
        Ok(Table {
            nome: nome.to_string(),
            diretorio,
            esquema,
            reg,
            ndx,
            bin,
            memo,
        })
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn diretorio(&self) -> &Path {
        &self.diretorio
    }

    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }

    pub fn registros(&self) -> u64 {
        self.reg.registros()
    }

    /// Maior rowid ja atribuido, incluindo os excluidos.
    pub fn slots(&self) -> u64 {
        self.reg.slots()
    }

    // ------------------------------------------------------- codificacao

    fn conferir_aridade(&self, valores: &[Value]) -> Result<()> {
        let n = self.esquema().colunas().len();
        if valores.len() != n {
            return Err(PhxError::Tipo(format!(
                "{}: esperado {n} valores, recebido {}",
                self.nome,
                valores.len()
            )));
        }
        Ok(())
    }

    /// Monta o payload do `.reg`, gravando antes o que vai para `.bin`/`.memo`.
    fn montar_payload(&mut self, valores: &[Value]) -> Result<Vec<u8>> {
        let mut payload = vec![0u8; self.esquema.payload_len()];

        for i in 0..self.esquema.colunas().len() {
            let col = &self.esquema.colunas()[i];
            let valor = &valores[i];
            if valor.e_null() {
                if !col.nullable {
                    return Err(PhxError::Tipo(format!(
                        "coluna {} e obrigatoria e recebeu NULL",
                        col.nome
                    )));
                }
                payload[i / 8] |= 1 << (i % 8);
                continue;
            }
            let off = self.esquema.offset_coluna(i)?;
            let fim = off + col.ty.largura();
            let ty = col.ty;
            let nome_col = col.nome.clone();
            match ty {
                ColumnType::Bin => {
                    let dados = match valor {
                        Value::Bin(b) => b.clone(),
                        outro => {
                            return Err(PhxError::Tipo(format!(
                                "coluna {nome_col} espera Bin, recebeu {outro:?}"
                            )))
                        }
                    };
                    let p = self.bin.gravar(&dados)?;
                    p.escrever(&mut payload[off..fim])?;
                }
                ColumnType::Memo => {
                    let texto = match valor {
                        Value::Memo(s) | Value::Str(s) => s.clone(),
                        outro => {
                            return Err(PhxError::Tipo(format!(
                                "coluna {nome_col} espera Memo, recebeu {outro:?}"
                            )))
                        }
                    };
                    let p = self.memo.gravar(texto.as_bytes())?;
                    p.escrever(&mut payload[off..fim])?;
                }
                _ => escrever_inline(valor, &ty, &mut payload[off..fim])?,
            }
        }
        Ok(payload)
    }

    /// Le o payload de volta. Se `carregar_externos` for falso, colunas
    /// `Bin`/`Memo` voltam como `Value::Null` -- util quando so precisamos
    /// dos valores que entram em indice.
    fn decodificar(&mut self, payload: &[u8], carregar_externos: bool) -> Result<Linha> {
        let mut linha = Vec::with_capacity(self.esquema.colunas().len());

        for i in 0..self.esquema.colunas().len() {
            if payload[i / 8] & (1 << (i % 8)) != 0 {
                linha.push(Value::Null);
                continue;
            }
            let ty = self.esquema.colunas()[i].ty;
            let off = self.esquema.offset_coluna(i)?;
            let fim = off + ty.largura();
            let valor = match ty {
                ColumnType::Bin => {
                    if !carregar_externos {
                        Value::Null
                    } else {
                        let p = Ponteiro::ler(&payload[off..fim])?;
                        Value::Bin(self.bin.ler(&p)?)
                    }
                }
                ColumnType::Memo => {
                    if !carregar_externos {
                        Value::Null
                    } else {
                        let p = Ponteiro::ler(&payload[off..fim])?;
                        let bytes = self.memo.ler(&p)?;
                        Value::Memo(String::from_utf8(bytes).map_err(|e| {
                            PhxError::Corrompido(format!("memo nao e UTF-8 valido: {e}"))
                        })?)
                    }
                }
                _ => ler_inline(&ty, &payload[off..fim])?,
            };
            linha.push(valor);
        }
        Ok(linha)
    }

    /// Ponteiros externos guardados num payload, para poder liberar depois.
    fn ponteiros(&self, payload: &[u8]) -> Result<Vec<(ColumnType, Ponteiro)>> {
        let esquema = &self.esquema;
        let mut saida = Vec::new();
        for (i, col) in esquema.colunas().iter().enumerate() {
            if !col.ty.externo() || payload[i / 8] & (1 << (i % 8)) != 0 {
                continue;
            }
            let off = esquema.offset_coluna(i)?;
            saida.push((
                col.ty,
                Ponteiro::ler(&payload[off..off + col.ty.largura()])?,
            ));
        }
        Ok(saida)
    }

    fn liberar_externos(&mut self, ponteiros: &[(ColumnType, Ponteiro)]) -> Result<()> {
        for (ty, p) in ponteiros {
            match ty {
                ColumnType::Bin => self.bin.liberar(p)?,
                ColumnType::Memo => self.memo.liberar(p)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Codifica a chave do indice `idx` a partir dos valores da linha.
    fn codificar_chave(&self, idx: usize, valores: &[Value]) -> Result<Vec<u8>> {
        let esquema = &self.esquema;
        let def = &esquema.indices()[idx];
        let mut chave = Vec::new();
        for ic in &def.colunas {
            let col = &esquema.colunas()[ic.coluna];
            let n = largura_componente(&col.ty)?;
            let base = chave.len();
            chave.resize(base + n, 0);
            escrever_componente(
                &valores[ic.coluna],
                &col.ty,
                ic.desc,
                ic.nocase,
                &mut chave[base..base + n],
            )?;
        }
        Ok(chave)
    }

    fn todas_as_chaves(&self, valores: &[Value]) -> Result<Vec<Vec<u8>>> {
        (0..self.esquema.indices().len())
            .map(|i| self.codificar_chave(i, valores))
            .collect()
    }

    // ------------------------------------------------------------ escrita

    /// Insere uma linha e devolve o rowid.
    ///
    /// A checagem de indice unico acontece ANTES de tocar no `.reg`; se um
    /// indice falhar no meio do caminho, o que ja foi gravado e desfeito.
    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {
        self.conferir_aridade(valores)?;
        let chaves = self.todas_as_chaves(valores)?;

        for (i, chave) in chaves.iter().enumerate() {
            if self.ndx.indices()[i].unico && !self.ndx.buscar(i, chave)?.is_empty() {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {} ja tem essa chave",
                    self.ndx.indices()[i].nome
                )));
            }
        }

        let payload = self.montar_payload(valores)?;
        let ponteiros = self.ponteiros(&payload)?;
        let rowid = self.reg.inserir(&payload)?;

        for (i, chave) in chaves.iter().enumerate() {
            if let Err(e) = self.ndx.inserir(i, chave, rowid) {
                // Desfaz o que ja entrou.
                for (j, anterior) in chaves.iter().enumerate().take(i) {
                    let _ = self.ndx.remover(j, anterior, rowid);
                }
                let _ = self.reg.excluir(rowid);
                let _ = self.liberar_externos(&ponteiros);
                return Err(e);
            }
        }
        Ok(rowid)
    }

    /// Le uma linha completa, carregando `.bin` e `.memo`.
    pub fn ler(&mut self, rowid: RowId) -> Result<Option<Linha>> {
        match self.reg.ler(rowid)? {
            None => Ok(None),
            Some(payload) => Ok(Some(self.decodificar(&payload, true)?)),
        }
    }

    /// Regrava a linha inteira mantendo o mesmo rowid e a mesma posicao
    /// fisica no `.reg`.
    pub fn atualizar(&mut self, rowid: RowId, valores: &[Value]) -> Result<()> {
        self.conferir_aridade(valores)?;
        let antigo = self
            .reg
            .ler(rowid)?
            .ok_or_else(|| PhxError::NaoEncontrado(format!("registro {rowid} esta excluido")))?;

        let valores_antigos = self.decodificar(&antigo, false)?;
        let chaves_antigas = self.todas_as_chaves(&valores_antigos)?;
        let chaves_novas = self.todas_as_chaves(valores)?;

        // Unicidade: so reclama se a chave mudou e ja pertence a outro rowid.
        for (i, nova) in chaves_novas.iter().enumerate() {
            if !self.ndx.indices()[i].unico || *nova == chaves_antigas[i] {
                continue;
            }
            let donos = self.ndx.buscar(i, nova)?;
            if donos.iter().any(|&r| r != rowid) {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {} ja tem essa chave",
                    self.ndx.indices()[i].nome
                )));
            }
        }

        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        self.reg.atualizar(rowid, &payload)?;

        for (i, (antiga, nova)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {
            if antiga != nova {
                self.ndx.remover(i, antiga, rowid)?;
                self.ndx.inserir(i, nova, rowid)?;
            }
        }
        self.liberar_externos(&ponteiros_antigos)?;
        Ok(())
    }

    /// Exclui a linha: tira as chaves dos indices, libera os blocos externos
    /// e marca o slot do `.reg` como livre.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        let payload = match self.reg.ler(rowid)? {
            None => return Ok(false),
            Some(p) => p,
        };
        let valores = self.decodificar(&payload, false)?;
        let chaves = self.todas_as_chaves(&valores)?;
        for (i, chave) in chaves.iter().enumerate() {
            self.ndx.remover(i, chave, rowid)?;
        }
        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        self.reg.excluir(rowid)
    }

    // ------------------------------------------------------------ leitura

    /// Percorre a tabela na ORDEM DE DIGITACAO, direto do `.reg`.
    pub fn varrer(&mut self) -> Result<Vec<(RowId, Linha)>> {
        let mut saida = Vec::new();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            saida.push((id, self.decodificar(&payload, true)?));
            rowid = id + 1;
        }
        Ok(saida)
    }

    fn idx_por_nome(&self, indice: &str) -> Result<usize> {
        self.ndx
            .indice_por_nome(indice)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} nao existe")))
    }

    /// Rowids com a chave exata, em ordem de digitacao dentro da chave.
    pub fn buscar(&mut self, indice: &str, chave: &[Value]) -> Result<Vec<RowId>> {
        let i = self.idx_por_nome(indice)?;
        let valores = self.espalhar(i, chave)?;
        let codificada = self.codificar_chave(i, &valores)?;
        self.ndx.buscar(i, &codificada)
    }

    /// Rowids no intervalo de chaves `[de, ate]`, na ordem do indice.
    pub fn intervalo(
        &mut self,
        indice: &str,
        de: Option<&[Value]>,
        ate: Option<&[Value]>,
    ) -> Result<Vec<RowId>> {
        let i = self.idx_por_nome(indice)?;
        let de = match de {
            Some(v) => Some(self.codificar_chave(i, &self.espalhar(i, v)?)?),
            None => None,
        };
        let ate = match ate {
            Some(v) => Some(self.codificar_chave(i, &self.espalhar(i, v)?)?),
            None => None,
        };
        self.ndx.intervalo(i, de.as_deref(), ate.as_deref())
    }

    /// Todos os rowids na ordem do indice.
    pub fn varrer_indice(&mut self, indice: &str) -> Result<Vec<RowId>> {
        let i = self.idx_por_nome(indice)?;
        self.ndx.varrer(i)
    }

    /// Recebe os valores na ordem das colunas do INDICE e devolve um vetor
    /// no formato de linha, para reaproveitar `codificar_chave`.
    fn espalhar(&self, idx: usize, chave: &[Value]) -> Result<Linha> {
        let esquema = &self.esquema;
        let def = &esquema.indices()[idx];
        if chave.len() != def.colunas.len() {
            return Err(PhxError::Tipo(format!(
                "indice {} tem {} colunas, recebeu {} valores",
                def.nome,
                def.colunas.len(),
                chave.len()
            )));
        }
        let mut linha = vec![Value::Null; esquema.colunas().len()];
        for (ic, v) in def.colunas.iter().zip(chave.iter()) {
            linha[ic.coluna] = v.clone();
        }
        Ok(linha)
    }

    // ------------------------------------------------------- manutencao

    /// Confere a integridade das quatro pecas: CRC de cada registro, CRC e
    /// ordenacao de cada pagina de indice, e CRC de cada bloco externo.
    pub fn verificar(&mut self) -> Result<Relatorio> {
        let registros = self.reg.verificar()?;
        let indices = self.ndx.verificar()?;
        let blocos_bin = self.bin.verificar()?;
        let blocos_memo = self.memo.verificar()?;

        for (nome, qtd) in &indices {
            if *qtd != registros {
                return Err(PhxError::Corrompido(format!(
                    "{}: indice {nome} tem {qtd} chaves para {registros} registros",
                    self.nome
                )));
            }
        }

        Ok(Relatorio {
            tabela: self.nome.clone(),
            registros,
            slots: self.reg.slots(),
            indices,
            blocos_bin,
            blocos_memo,
        })
    }

    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(
        &self,
    ) -> (crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob) {
        (self.bin.estatistica(), self.memo.estatistica())
    }

    /// Paginas ocupadas pelo `.ndx`, incluindo a pagina 0 de cabecalho.
    pub fn paginas_indice(&self) -> u64 {
        self.ndx.paginas()
    }

    /// Descritores dos indices como estao gravados no `.ndx`.
    pub fn descritores_indices(&self) -> &[crate::ndx::DescritorIndice] {
        self.ndx.indices()
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.reg.sincronizar()?;
        self.ndx.sincronizar()?;
        self.bin.sincronizar()?;
        self.memo.sincronizar()?;
        Ok(())
    }
}
