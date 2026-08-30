# Wire the two new files into Table
# 28/08 17:30

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

# --- cabecalho do modulo e imports
s = s.replace('''//! `Table` -- a tabela de dados, que e a soma dos quatro arquivos.
//!
//! ```text
//! cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  =  cadastroClientes
//! ```''','''//! `Table` -- a tabela de dados, que e a soma dos seus arquivos.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
//! ```
//!
//! Mais o espelho `.bkp`, quando ligado.''',1)

s = s.replace('''use phxsql_core::schema::Schema;''','''use phxsql_core::schema::Schema;''',1)
s = s.replace('''use crate::blob::{BlobFile, MAGIC_BIN, MAGIC_MEMO};
use crate::log::{Evento, LogFile, Operacao, EXT_LOG};
use crate::ndx::NdxFile;
use crate::reg::RegFile;''','''use crate::blob::{BlobFile, MAGIC_BIN, MAGIC_MEMO};
use crate::lixeira::{Descartada, LixeiraFile, EXT_TRASH};
use crate::log::{Evento, LogFile, Operacao, EXT_LOG};
use crate::motivo::{Motivo, MotivoFile, Tipo, EXT_REASON};
use crate::ndx::NdxFile;
use crate::reg::RegFile;''',1)

# --- campos da struct
s = s.replace('''    bin: BlobFile,
    memo: BlobFile,
    log: LogFile,
}''','''    bin: BlobFile,
    memo: BlobFile,
    log: LogFile,
    lixeira: LixeiraFile,
    motivos: MotivoFile,
}''',1)

# --- criar
s = s.replace('''        for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO, EXT_LOG] {''',
              '''        for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO, EXT_LOG, EXT_TRASH, EXT_REASON] {''',1)
s = s.replace('''        let log = LogFile::criar(&diretorio, &nome, paginacao)?;
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
    }''','''        let log = LogFile::criar(&diretorio, &nome, paginacao)?;
        let lixeira = LixeiraFile::criar(&diretorio, &nome, paginacao)?;
        let motivos = MotivoFile::criar(&diretorio, &nome, paginacao)?;
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
            lixeira,
            motivos,
        })
    }''',1)

# --- abrir
s = s.replace('''        let log = LogFile::abrir(&diretorio, nome, paginacao)?;

        if ndx.indices().len()''','''        let log = LogFile::abrir(&diretorio, nome, paginacao)?;
        // `abrir` destes dois CRIA quando falta: tabela feita antes deles
        // existirem tem de continuar abrindo.
        let lixeira = LixeiraFile::abrir(&diretorio, nome, paginacao)?;
        let motivos = MotivoFile::abrir(&diretorio, nome, paginacao)?;

        if ndx.indices().len()''',1)
s = s.replace('''            reg,
            ndx,
            bin,
            memo,
            log,
        })
    }

    pub fn nome(&self) -> &str {''','''            reg,
            ndx,
            bin,
            memo,
            log,
            lixeira,
            motivos,
        })
    }

    pub fn nome(&self) -> &str {''',1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
