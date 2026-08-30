# Add foreign keys and pagination to Schema
# 27/08 18:19

p='crates/phxsql-core/src/schema.rs'
s=open(p).read()

s=s.replace('''use crate::error::{PhxError, Result};
use crate::keyenc::largura_componente;
use crate::types::ColumnType;

const MAGIC_ESQUEMA: &[u8; 4] = b"PSCH";
const VERSAO_ESQUEMA: u16 = 1;''','''use crate::error::{PhxError, Result};
use crate::keyenc::largura_componente;
use crate::paginacao::Paginacao;
use crate::types::ColumnType;

const MAGIC_ESQUEMA: &[u8; 4] = b"PSCH";
const VERSAO_ESQUEMA: u16 = 2;

/// O que fazer com as linhas filhas quando a linha pai muda ou some.
///
/// Mesma semantica do `RELATION` do dicionario do Clarion e do
/// `ON DELETE` / `ON UPDATE` do SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcaoRi {
    /// Nao mexe nas filhas. O banco pode ficar com referencia orfa.
    #[default]
    NaoFazerNada,
    /// Recusa a operacao enquanto existir filha.
    Restringir,
    /// Repete a operacao nas filhas.
    Cascata,
    /// Anula as colunas da filha que apontavam para o pai.
    AnularCampos,
}

impl AcaoRi {
    fn tag(self) -> u8 {
        match self {
            AcaoRi::NaoFazerNada => 0,
            AcaoRi::Restringir => 1,
            AcaoRi::Cascata => 2,
            AcaoRi::AnularCampos => 3,
        }
    }

    fn de_tag(t: u8) -> Result<AcaoRi> {
        Ok(match t {
            0 => AcaoRi::NaoFazerNada,
            1 => AcaoRi::Restringir,
            2 => AcaoRi::Cascata,
            3 => AcaoRi::AnularCampos,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "acao de integridade referencial desconhecida: {outro}"
                )))
            }
        })
    }
}

/// Chave estrangeira: liga colunas desta tabela a colunas de outra.
///
/// O FraseSQL precisa dessa informacao no catalogo para conseguir gerar JOIN;
/// e ela e tambem o `RELATION` do dicionario do Clarion, com CASCADE e
/// RESTRICT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub nome: String,
    /// Posicoes das colunas locais em [`Schema::colunas`].
    pub colunas: Vec<usize>,
    /// Tabela referenciada. Aceita nome simples ou `schema.tabela`.
    pub tabela_ref: String,
    /// Nomes das colunas na tabela referenciada, na mesma ordem.
    pub colunas_ref: Vec<String>,
    pub ao_excluir: AcaoRi,
    pub ao_alterar: AcaoRi,
}

impl ForeignKey {
    pub fn new(
        nome: impl Into<String>,
        colunas: Vec<usize>,
        tabela_ref: impl Into<String>,
        colunas_ref: Vec<String>,
    ) -> ForeignKey {
        ForeignKey {
            nome: nome.into(),
            colunas,
            tabela_ref: tabela_ref.into(),
            colunas_ref,
            ao_excluir: AcaoRi::Restringir,
            ao_alterar: AcaoRi::Restringir,
        }
    }

    pub fn ao_excluir(mut self, acao: AcaoRi) -> Self {
        self.ao_excluir = acao;
        self
    }

    pub fn ao_alterar(mut self, acao: AcaoRi) -> Self {
        self.ao_alterar = acao;
        self
    }
}''')

s=s.replace('''pub struct Schema {
    nome: String,
    colunas: Vec<Column>,
    indices: Vec<IndexDef>,
    offsets: Vec<usize>,
    bitmap_len: usize,
    payload_len: usize,
}''','''pub struct Schema {
    nome: String,
    colunas: Vec<Column>,
    indices: Vec<IndexDef>,
    chaves_estrangeiras: Vec<ForeignKey>,
    paginacao: Paginacao,
    offsets: Vec<usize>,
    bitmap_len: usize,
    payload_len: usize,
}''')

s=s.replace('''        Ok(Schema {
            nome,
            colunas,
            indices,
            offsets,
            bitmap_len,
            payload_len: pos,
        })
    }''','''        Ok(Schema {
            nome,
            colunas,
            indices,
            chaves_estrangeiras: Vec::new(),
            paginacao: Paginacao::DESLIGADA,
            offsets,
            bitmap_len,
            payload_len: pos,
        })
    }

    /// Acrescenta as chaves estrangeiras da tabela.
    pub fn com_chaves_estrangeiras(mut self, fks: Vec<ForeignKey>) -> Result<Schema> {
        for (i, fk) in fks.iter().enumerate() {
            if fk.nome.is_empty() {
                return Err(PhxError::Esquema(format!("chave estrangeira {i} sem nome")));
            }
            if fks.iter().take(i).any(|o| o.nome == fk.nome) {
                return Err(PhxError::Esquema(format!(
                    "chave estrangeira duplicada: {}",
                    fk.nome
                )));
            }
            if fk.colunas.is_empty() {
                return Err(PhxError::Esquema(format!("{} sem colunas", fk.nome)));
            }
            if fk.colunas.len() != fk.colunas_ref.len() {
                return Err(PhxError::Esquema(format!(
                    "{}: {} colunas locais para {} referenciadas",
                    fk.nome,
                    fk.colunas.len(),
                    fk.colunas_ref.len()
                )));
            }
            if fk.tabela_ref.trim().is_empty() {
                return Err(PhxError::Esquema(format!(
                    "{} nao diz qual tabela referencia",
                    fk.nome
                )));
            }
            for c in &fk.colunas {
                if *c >= self.colunas.len() {
                    return Err(PhxError::Esquema(format!(
                        "{} referencia coluna inexistente {c}",
                        fk.nome
                    )));
                }
            }
        }
        self.chaves_estrangeiras = fks;
        Ok(self)
    }

    /// Liga a paginacao da tabela (os numeros do `CREATE TABLE`).
    pub fn com_paginacao(mut self, paginacao: Paginacao) -> Schema {
        self.paginacao = paginacao;
        self
    }

    pub fn chaves_estrangeiras(&self) -> &[ForeignKey] {
        &self.chaves_estrangeiras
    }

    pub fn paginacao(&self) -> Paginacao {
        self.paginacao
    }''')

# --- serializacao v2 ---
s=s.replace('''            for ic in &idx.colunas {
                out.extend_from_slice(&(ic.coluna as u16).to_le_bytes());
                out.push((ic.desc as u8) | ((ic.nocase as u8) << 1));
            }
        }
        out
    }''','''            for ic in &idx.colunas {
                out.extend_from_slice(&(ic.coluna as u16).to_le_bytes());
                out.push((ic.desc as u8) | ((ic.nocase as u8) << 1));
            }
        }

        out.extend_from_slice(&(self.chaves_estrangeiras.len() as u16).to_le_bytes());
        for fk in &self.chaves_estrangeiras {
            escrever_texto(&mut out, &fk.nome);
            escrever_texto(&mut out, &fk.tabela_ref);
            out.push(fk.ao_excluir.tag());
            out.push(fk.ao_alterar.tag());
            out.extend_from_slice(&(fk.colunas.len() as u16).to_le_bytes());
            for c in &fk.colunas {
                out.extend_from_slice(&(*c as u16).to_le_bytes());
            }
            for c in &fk.colunas_ref {
                escrever_texto(&mut out, c);
            }
        }

        let p = self.paginacao;
        out.extend_from_slice(&p.registros_por_arquivo.to_le_bytes());
        out.extend_from_slice(&p.max_arquivos.to_le_bytes());
        out.push(p.digitos);
        out.extend_from_slice(&p.bytes_por_arquivo.to_le_bytes());
        out
    }''')

s=s.replace('''            indices.push(IndexDef {
                nome,
                colunas: cols,
                unico,
            });
        }

        Schema::new(nome, colunas, indices)
    }''','''            indices.push(IndexDef {
                nome,
                colunas: cols,
                unico,
            });
        }

        let n_fk = leitor.u16()? as usize;
        let mut fks = Vec::with_capacity(n_fk);
        for _ in 0..n_fk {
            let nome_fk = leitor.texto()?;
            let tabela_ref = leitor.texto()?;
            let ao_excluir = AcaoRi::de_tag(leitor.u8()?)?;
            let ao_alterar = AcaoRi::de_tag(leitor.u8()?)?;
            let n = leitor.u16()? as usize;
            let mut cols = Vec::with_capacity(n);
            for _ in 0..n {
                cols.push(leitor.u16()? as usize);
            }
            let mut cols_ref = Vec::with_capacity(n);
            for _ in 0..n {
                cols_ref.push(leitor.texto()?);
            }
            fks.push(ForeignKey {
                nome: nome_fk,
                colunas: cols,
                tabela_ref,
                colunas_ref: cols_ref,
                ao_excluir,
                ao_alterar,
            });
        }

        let paginacao = Paginacao {
            registros_por_arquivo: leitor.u64()?,
            max_arquivos: leitor.u32()?,
            digitos: leitor.u8()?,
            bytes_por_arquivo: leitor.u64()?,
        };

        Schema::new(nome, colunas, indices)?
            .com_chaves_estrangeiras(fks)
            .map(|e| e.com_paginacao(paginacao))
    }''')

s=s.replace('''    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }''','''    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }''')
open(p,'w').write(s)
print("schema.rs: FK + paginacao")
