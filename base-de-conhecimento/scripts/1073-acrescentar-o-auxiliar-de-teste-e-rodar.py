# Acrescentar o auxiliar de teste e rodar
# 29/08 05:19

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
anc='''    /// Onde a ultima varredura parou. Ver [`MarcaDoDiario`].'''
velho='''    pub fn estatisticas_paginas(&self) -> (u64, u64, u64) {'''
assert s.count(velho)==1
novo='''    /// Poe o contador de chaves num valor qualquer. **So para teste.**
    ///
    /// Existe porque a conferencia precisa provar que ela ainda para quando a
    /// varredura acha MENOS chaves do que o diretorio diz -- e nao ha como
    /// chegar nesse estado por fora sem corromper o arquivo a mao.
    #[doc(hidden)]
    pub fn forjar_contador_para_teste(&mut self, idx: usize, qtd: u64) {
        if let Some(d) = self.indices.get_mut(idx) {
            d.qtd_chaves = qtd;
        }
    }

    pub fn estatisticas_paginas(&self) -> (u64, u64, u64) {'''
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
