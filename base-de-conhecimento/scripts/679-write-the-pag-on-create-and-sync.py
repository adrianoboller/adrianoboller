# Write the .pag on create and sync
# 28/08 18:50

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
# criar tambem escreve o .pag, e a criacao ja recusa arquivo existente
s=s.replace('''        for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO, EXT_LOG, EXT_TRASH, EXT_REASON] {''',
            '''        for ext in [
            EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO, EXT_LOG, EXT_TRASH, EXT_REASON,
        ] {''',1)
s=s.replace('''        let reg = RegFile::criar(&diretorio, &nome, esquema.clone())?;

        Ok(Table {''','''        let reg = RegFile::criar(&diretorio, &nome, esquema.clone())?;

        let mut t = Table {''',1)
s=s.replace('''            log,
            lixeira,
            motivos,
        })
    }

    /// Abre uma tabela existente.''','''            log,
            lixeira,
            motivos,
        };
        t.gravar_pag()?;
        Ok(t)
    }

    /// Abre uma tabela existente.''',1)
io.open(p,'w',encoding='utf-8').write(s)
