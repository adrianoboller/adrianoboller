# Pin the read-only classification
# 28/08 17:40

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            "sessoes",
            "sistema",
            "dblink",'''
novo='''            "sessoes",
            "sistema",
            // Listar a lixeira e os motivos so le -- e e exatamente o que se
            // quer poder fazer num espelho somente-leitura, investigando.
            "lixeira",
            "motivos",
            "dblink",'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
