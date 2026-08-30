# Write the .pag on create and sync
# 28/08 18:50

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// Quantas linhas cada balde tem. Vazio fora da particao alfanumerica.
    pub fn baldes(&self) -> &[u64] {
        self.reg.baldes()
    }'''
novo='''    /// Quantas linhas cada balde tem. Vazio fora da particao alfanumerica.
    pub fn baldes(&self) -> &[u64] {
        self.reg.baldes()
    }

    /// Regrava o `.pag`, o descritor de particao da tabela.
    ///
    /// Gerado, e nunca lido pelo motor: a verdade continua no bloco de esquema
    /// do `.reg` e nos cabecalhos dos volumes. Ver [`crate::pag`].
    pub fn gravar_pag(&mut self) -> Result<std::path::PathBuf> {
        let volumes = self.reg.volumes();
        crate::pag::escrever(
            &self.diretorio,
            &self.nome,
            &self.esquema,
            self.reg.baldes(),
            &volumes,
        )
    }'''
assert velho in s
s=s.replace(velho,novo,1)

# o .pag nasce com a tabela e acompanha o sincronizar
s=s.replace('''        self.lixeira.sincronizar()?;
        self.motivos.sincronizar()?;
        Ok(())''','''        self.lixeira.sincronizar()?;
        self.motivos.sincronizar()?;
        // O descritor acompanha o disco: ele so vale se disser o que os
        // arquivos dizem, e o `sincronizar` e justamente o instante em que os
        // arquivos param de mudar.
        self.gravar_pag()?;
        Ok(())''',1)
io.open(p,'w',encoding='utf-8').write(s)
