# Single-pass filter by visao
# 28/08 17:49

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// Tira da lista os rowids de linhas marcadas como excluidas.
    ///
    /// Os caminhos por indice devolvem rowid, e a marca esta no registro:
    /// filtrar exige ler cada um. Numa tabela sem a coluna de sistema a lista
    /// volta como veio, sem leitura nenhuma.
    pub fn filtrar_ativos(&mut self, rowids: &[RowId]) -> Result<Vec<RowId>> {
        if self.esquema.coluna_softdeleted().is_none() {
            return Ok(rowids.to_vec());
        }
        let mut saida = Vec::with_capacity(rowids.len());
        for &r in rowids {
            if let Some(p) = self.reg.ler(r)? {
                let linha = self.decodificar(&p, false)?;
                if !self.esta_excluida(&linha) {
                    saida.push(r);
                }
            }
        }
        Ok(saida)
    }'''
novo='''    /// Tira da lista os rowids que a visao nao enxerga.
    ///
    /// Os caminhos por indice devolvem rowid, e a marca esta no registro:
    /// filtrar exige ler cada um. Numa passada so -- ler duas vezes para
    /// depois cruzar as duas listas custaria o dobro de leitura e uma busca
    /// linear por elemento.
    ///
    /// Numa tabela sem a coluna de sistema nao ha o que marcar: a lista volta
    /// como veio, sem leitura nenhuma, e `Excluidas` volta vazia.
    pub fn filtrar(&mut self, rowids: &[RowId], visao: Visao) -> Result<Vec<RowId>> {
        if visao == Visao::Todas {
            return Ok(rowids.to_vec());
        }
        if self.esquema.coluna_softdeleted().is_none() {
            return Ok(match visao {
                Visao::Excluidas => Vec::new(),
                _ => rowids.to_vec(),
            });
        }
        let mut saida = Vec::with_capacity(rowids.len());
        for &r in rowids {
            if let Some(p) = self.reg.ler(r)? {
                let linha = self.decodificar(&p, false)?;
                if visao.aceita(self.esta_excluida(&linha)) {
                    saida.push(r);
                }
            }
        }
        Ok(saida)
    }

    /// Atalho para a visao comum. Ver [`Table::filtrar`].
    pub fn filtrar_ativos(&mut self, rowids: &[RowId]) -> Result<Vec<RowId>> {
        self.filtrar(rowids, Visao::Ativas)
    }'''
assert velho in s
s=s.replace(velho,novo,1)
# `aceita` passa a ser visivel dentro do crate
s=s.replace('''impl Visao {
    fn aceita(self, excluida: bool) -> bool {''','''impl Visao {
    /// Esta linha entra nesta visao?
    pub fn aceita(self, excluida: bool) -> bool {''',1)
io.open(p,'w',encoding='utf-8').write(s)
