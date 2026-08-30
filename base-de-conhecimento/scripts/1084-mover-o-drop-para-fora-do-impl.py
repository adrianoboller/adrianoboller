# Mover o Drop para fora do impl
# 29/08 06:03

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
bloco='''
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
assert s.count(bloco)==1
s=s.replace(bloco,'')
# ao fim do arquivo, fora do impl
s = s.rstrip('\n') + '\n' + bloco
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
