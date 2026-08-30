# Validate the alphanumeric partition schema
# 28/08 18:48

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        self.paginacao = paginacao;
        Ok(self)
    }

    /// Fixa a paginacao sem conferir'''
novo='''        if let ModoParticao::PorLetra { coluna } = paginacao.modo {
            let c = self.colunas.get(coluna as usize).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a particao alfanumerica aponta a coluna {coluna}, \\
                     que nao existe em {}",
                    self.nome
                ))
            })?;
            // Coluna externa nao serve: o valor dela nao esta no slot, e
            // decidir o arquivo de destino exigiria ler o `.memo` antes de
            // saber em que arquivo gravar -- que e a ordem invertida.
            if c.ty.externo() {
                return Err(PhxError::Esquema(format!(
                    "a particao alfanumerica nao pode apontar {}, que e {:?}: \\
                     o valor mora fora do slot, e o balde precisa ser decidido \\
                     ANTES de a linha ser gravada",
                    c.nome, c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna de particao {} aceita nulo; a linha sem valor \\
                     cairia toda no balde Outros sem ninguem ter escolhido isso",
                    c.nome
                )));
            }
            if paginacao.max_arquivos as usize != BALDES.len() {
                return Err(PhxError::Esquema(format!(
                    "a particao alfanumerica tem exatamente {} volumes \\
                     (A-Z, 0-9 e Outros); o esquema pede {}",
                    BALDES.len(),
                    paginacao.max_arquivos
                )));
            }
        }
        self.paginacao = paginacao;
        Ok(self)
    }

    /// Fixa a paginacao sem conferir'''
assert velho in s
s=s.replace(velho,novo,1)
s=s.replace("use crate::paginacao::{ModoParticao, Paginacao};","use crate::paginacao::{ModoParticao, Paginacao, BALDES};",1)
io.open(p,'w',encoding='utf-8').write(s)
