# Fechamento limpo e rede no Drop
# 29/08 06:03

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
anc='''    /// O arquivo foi aberto com a marca de sujo levantada.'''
assert s.count(anc)==1
novo='''    /// Leva as paginas sujas ao arquivo e baixa a marca, SEM `fsync`.
    ///
    /// E o fechamento limpo: o `write` ja entregou tudo ao nucleo, entao uma
    /// queda do PROCESSO nao perde nada -- que era a garantia de antes do
    /// write-back, e ela volta inteira aqui. Quem quer resistir a queda da
    /// MAQUINA chama `sincronizar`, que acrescenta os dois `fsync`.
    ///
    /// E o mesmo momento em que o Aria baixa a marca dele: ao destravar a
    /// tabela (`ma_locking.c:301`), e nao a cada linha.
    pub fn fechar(&mut self) -> Result<()> {
        self.descarregar()?;
        if self.sujo || self.estrutura_mudou {
            self.sujo = false;
            self.gravar_cabecalho()?;
        }
        Ok(())
    }

    /// O arquivo foi aberto com a marca de sujo levantada.'''
s=s.replace(anc,novo)

# `sincronizar` passa a se apoiar no `fechar`, sem duplicar a ordem
velho='''    pub fn sincronizar(&mut self) -> Result<()> {
        self.descarregar()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;

        self.sujo = false;
        self.gravar_cabecalho()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }'''
novo2='''    pub fn sincronizar(&mut self) -> Result<()> {
        self.descarregar()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;

        self.sujo = false;
        self.gravar_cabecalho()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }

impl Drop for NdxFile {
    /// Rede de seguranca do fechamento limpo.
    ///
    /// Quem esquecer de chamar `fechar` ou `sincronizar` ainda tem as paginas
    /// levadas ao arquivo aqui. E se a gravacao FALHAR, a marca de sujo fica
    /// levantada -- que e a resposta certa: o indice realmente nao presta, e a
    /// proxima abertura vai dizer isso em vez de responder errado.
    fn drop(&mut self) {
        let _ = self.fechar();
    }
}
'''
assert s.count(velho)==1
s=s.replace(velho,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
