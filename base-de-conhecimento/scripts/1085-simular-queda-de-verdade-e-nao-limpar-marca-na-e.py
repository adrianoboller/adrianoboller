# Simular queda de verdade e nao limpar marca na espiada
# 29/08 06:04

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    pub fn fechar(&mut self) -> Result<()> {
        self.descarregar()?;
        if self.sujo || self.estrutura_mudou {'''
novo='''    pub fn fechar(&mut self) -> Result<()> {
        // Um arquivo aberto JA sujo nao se limpa fechando: nada foi
        // reconstruido, e a arvore continua sem as chaves que faltam. So o
        // `reindexar`, que recria o arquivo, tira a marca -- senao bastaria
        // alguem abrir e fechar para o defeito virar invisivel.
        if self.precisa_reconstruir {
            return Ok(());
        }
        self.descarregar()?;
        if self.sujo || self.estrutura_mudou {'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ndx ok')
