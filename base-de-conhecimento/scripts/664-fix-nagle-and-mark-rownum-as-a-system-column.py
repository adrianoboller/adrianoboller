# Fix Nagle and mark rownum as a system column
# 28/08 18:39

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
# rownum tambem e coluna de sistema para a tela
s=s.replace('''                    ("sistema", Json::Bool(Some(i) == e.coluna_softdeleted())),''',
            '''                    (
                        "sistema",
                        Json::Bool(phxsql_core::schema::e_coluna_de_sistema(&c.nome)),
                    ),''',1)
io.open(p,'w',encoding='utf-8').write(s)
