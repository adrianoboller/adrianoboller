# Add write ops and rebuild
# 28/08 17:40

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    "ajustar_sequencia",'''
novo='''    "ajustar_sequencia",
    // Marcar, desmarcar e esvaziar mexem em dado gravado. Listar a lixeira e
    // os motivos, nao -- essas duas so leem, e continuam valendo no modo
    // somente leitura, que e justamente quando alguem esta investigando.
    "restaurar",
    "esvaziar_lixeira",'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
