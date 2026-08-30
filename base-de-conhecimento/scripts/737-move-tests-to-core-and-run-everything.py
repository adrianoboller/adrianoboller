# Move tests to core and run everything
# 28/08 19:32

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
# o bloco de testes numero_pt agora testa o que esta no nucleo
i=s.find('#[cfg(test)]\nmod testes_numero_pt {')
if i>0:
    s=s[:i].rstrip()+'\n'
io.open(p,'w',encoding='utf-8').write(s)
