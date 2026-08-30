# Wire the server to core's conversion
# 28/08 19:31

import io, re
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
# remove as duas duplicadas do servidor
for nome in ('pub fn hex_para_bytes(', 'fn data_de_texto('):
    i=s.index(nome)
    # inclui o comentario de doc acima
    ini=s.rfind('\n\n', 0, i)+2
    fim=s.index('\n}\n', i)+3
    s=s[:ini]+s[fim:]
io.open(p,'w',encoding='utf-8').write(s)
