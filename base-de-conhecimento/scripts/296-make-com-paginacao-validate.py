# Make com_paginacao validate
# 28/08 11:17

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()
v = '''    pub fn com_paginacao(mut self, paginacao: Paginacao) -> Schema {
        self.paginacao = paginacao;
        self
    }'''
n = '''    /// Fixa a paginacao, conferindo o que ela promete sobre as colunas.
    ///
    /// A particao por periodo aponta uma coluna, e ela tem de existir e ser uma
    /// data. Conferir aqui, e nao na gravacao: um esquema que so quebra na
    /// primeira insercao ja nasceu quebrado, e o erro apareceria longe de quem
    /// o causou.
    pub fn com_paginacao(mut self, paginacao: Paginacao) -> Result<Schema> {
        if let ModoParticao::PorPeriodo { coluna, periodo } = paginacao.modo {
            let c = self.colunas.get(coluna as usize).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "particao {} aponta a coluna {coluna}, que nao existe em {}",
                    periodo.nome(),
                    self.nome
                ))
            })?;
            if !matches!(c.ty, ColumnType::Date | ColumnType::DateTime) {
                return Err(PhxError::Esquema(format!(
                    "particao {} pede uma coluna de data; {} e {:?}",
                    periodo.nome(),
                    c.nome,
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna de particao {} aceita nulo; sem data nao ha periodo \\
                     em que a linha caiba",
                    c.nome
                )));
            }
        }
        self.paginacao = paginacao;
        Ok(self)
    }

    /// Fixa a paginacao sem conferir -- so para reabrir o que ja esta no disco.
    ///
    /// O que foi gravado ja passou pela conferencia uma vez, e recusar na
    /// leitura transformaria um esquema antigo em tabela ilegivel.
    pub(crate) fn com_paginacao_do_disco(mut self, paginacao: Paginacao) -> Schema {
        self.paginacao = paginacao;
        self
    }'''
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace('.map(|e| e.com_paginacao(paginacao))', '.map(|e| e.com_paginacao_do_disco(paginacao))')
p.write_text(s)
print('ok')
