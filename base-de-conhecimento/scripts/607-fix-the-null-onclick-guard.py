# Fix the null onclick guard
# 28/08 18:00

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''  if (!novo) $("#btExcluir").onclick = ev => {'''
novo='''  // O botao nao existe na linha ja marcada -- ali o que se oferece e
  // restaurar. Sem esta guarda, ligar o clique estoura na abertura da ficha.
  if (!novo && !marcada) $("#btExcluir").onclick = ev => {'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
