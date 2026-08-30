# Por a guarda no portao comum
# 29/08 06:01

import io,re
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

# `descritor` e o gargalo comum de quase todas: por a guarda ali cobre tudo
# que fala de um indice, e nao depende de alguem lembrar de conferir.
velho='''    fn descritor(&self, idx: usize) -> Result<&DescritorIndice> {'''
assert s.count(velho)==1
novo='''    /// O descritor de um indice -- e o portao por onde toda operacao passa.
    ///
    /// A guarda de "ficou para tras numa queda" mora AQUI, e nao espalhada por
    /// `inserir`, `buscar`, `varrer`, `intervalo`, `remover` e `verificar`.
    /// Espalhada, a que alguem esquecesse viraria a porta dos fundos -- e a
    /// porta dos fundos de uma marca de confiabilidade e uma busca respondendo
    /// errado em silencio. E a mesma licao do portao de permissao.
    fn descritor(&self, idx: usize) -> Result<&DescritorIndice> {
        self.conferir_confiavel()?;'''
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
