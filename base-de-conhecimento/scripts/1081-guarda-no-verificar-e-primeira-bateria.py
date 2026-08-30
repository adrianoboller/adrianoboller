# Guarda no verificar e primeira bateria
# 29/08 06:01

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
# `verificar` nao passa por `descritor` -- ela le `self.indices[i]` direto.
velho='''    pub fn verificar(&mut self) -> Result<Vec<(String, u64)>> {
        let mut saida = Vec::new();'''
novo='''    pub fn verificar(&mut self) -> Result<Vec<(String, u64)>> {
        // Esta nao passa por `descritor`: le `self.indices[i]` direto.
        self.conferir_confiavel()?;
        let mut saida = Vec::new();'''
assert s.count(velho)==1
s=s.replace(velho,novo)
# `tem_suja` nao e usada: sai, em vez de ficar como codigo morto.
velho2='''    fn tem_suja(&self) -> bool {
        self.paginas.values().any(|e| e.suja)
    }

'''
assert s.count(velho2)==1
s=s.replace(velho2,'')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
