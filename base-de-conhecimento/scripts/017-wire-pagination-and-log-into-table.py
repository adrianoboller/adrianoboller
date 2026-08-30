# Wire pagination and log into Table
# 27/08 18:26

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
s=s.replace('''    pub fn atualizar(&mut self, rowid: RowId, payload: &[u8]) -> Result<()> {''','''    /// Devolve a nova versao do registro.
    pub fn atualizar(&mut self, rowid: RowId, payload: &[u8]) -> Result<u64> {''')
s=s.replace('''        let versao = Campos(&slot).u64(8);
        slot[..SLOT_CAB].fill(0);
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, versao.saturating_add(1));
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;
        self.gravar_cabecalho(1)
    }''','''        let versao = Campos(&slot).u64(8).saturating_add(1);
        slot[..SLOT_CAB].fill(0);
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, versao);
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;
        self.gravar_cabecalho(1)?;
        Ok(versao)
    }''')
s=s.replace('''        r.atualizar(id, &payload(&esq, 99)).unwrap();''','''        assert_eq!(r.atualizar(id, &payload(&esq, 99)).unwrap(), 2);''')
open(p,'w').write(s)

p='crates/phxsql-store/src/table.rs'
s=open(p).read()

s=s.replace('''use phxsql_core::{RowId, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};

use crate::blob::{BlobFile, MAGIC_BIN, MAGIC_MEMO};
use crate::ndx::NdxFile;
use crate::reg::RegFile;''','''use phxsql_core::{RowId, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};

use crate::blob::{BlobFile, MAGIC_BIN, MAGIC_MEMO};
use crate::log::{Evento, LogFile, Operacao, EXT_LOG};
use crate::ndx::NdxFile;
use crate::reg::RegFile;''')

s=s.replace('''pub struct Relatorio {
    pub tabela: String,
    pub registros: u64,
    pub slots: u64,
    pub indices: Vec<(String, u64)>,
    pub blocos_bin: (u64, u64),
    pub blocos_memo: (u64, u64),
}''','''pub struct Relatorio {
    pub tabela: String,
    pub registros: u64,
    pub slots: u64,
    pub indices: Vec<(String, u64)>,
    pub blocos_bin: (u64, u64),
    pub blocos_memo: (u64, u64),
    /// Eventos conferidos no `.log`.
    pub eventos: u64,
    /// Volumes de cada arquivo paginado: `.reg`, `.bin`, `.memo`, `.log`.
    pub volumes: (usize, usize, usize, usize),
}''')

s=s.replace('''    esquema: Schema,
    reg: RegFile,
    ndx: NdxFile,
    bin: BlobFile,
    memo: BlobFile,
}''','''    esquema: Schema,
    reg: RegFile,
    ndx: NdxFile,
    bin: BlobFile,
    memo: BlobFile,
    log: LogFile,
}''')

s=s.replace('''        for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO] {
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
    }''','''        let paginacao = esquema.paginacao();
        for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO, EXT_LOG] {
            for c in [
                caminho(&diretorio, &nome, ext),
                diretorio.join(format!("{nome}{}.{ext}", paginacao.sufixo(1))),
            ] {
                if c.exists() {
                    return Err(PhxError::Esquema(format!(
                        "{} ja existe; use Table::abrir",
                        c.display()
                    )));
                }
            }
        }

        let ndx = NdxFile::criar(caminho(&diretorio, &nome, EXT_NDX), &esquema)?;
        let bin = BlobFile::criar(&diretorio, &nome, EXT_BIN, MAGIC_BIN, paginacao)?;
        let memo = BlobFile::criar(&diretorio, &nome, EXT_MEMO, MAGIC_MEMO, paginacao)?;
        let log = LogFile::criar(&diretorio, &nome, paginacao)?;
        let reg = RegFile::criar(&diretorio, &nome, esquema.clone())?;

        Ok(Table {
            nome,
            diretorio,
            esquema,
            reg,
            ndx,
            bin,
            memo,
            log,
        })
    }''')

s=s.replace('''        let reg = RegFile::abrir(caminho(&diretorio, nome, EXT_REG))?;
        let ndx = NdxFile::abrir(caminho(&diretorio, nome, EXT_NDX))?;
        let bin = BlobFile::abrir(caminho(&diretorio, nome, EXT_BIN), MAGIC_BIN)?;
        let memo = BlobFile::abrir(caminho(&diretorio, nome, EXT_MEMO), MAGIC_MEMO)?;''','''        let reg = RegFile::abrir(&diretorio, nome)?;
        let paginacao = reg.esquema().paginacao();
        let ndx = NdxFile::abrir(caminho(&diretorio, nome, EXT_NDX))?;
        let bin = BlobFile::abrir(&diretorio, nome, EXT_BIN, MAGIC_BIN, paginacao)?;
        let memo = BlobFile::abrir(&diretorio, nome, EXT_MEMO, MAGIC_MEMO, paginacao)?;
        let log = LogFile::abrir(&diretorio, nome, paginacao)?;''')

s=s.replace('''            reg,
            ndx,
            bin,
            memo,
        })
    }

    pub fn nome(&self) -> &str {''','''            reg,
            ndx,
            bin,
            memo,
            log,
        })
    }

    pub fn nome(&self) -> &str {''')
open(p,'w').write(s)
print("table.rs: assinaturas e log ligados")
