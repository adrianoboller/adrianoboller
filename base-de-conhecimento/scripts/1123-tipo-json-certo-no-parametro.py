# Tipo JSON certo no parametro
# 29/08 11:39

import io
p='crates/phxsql-server/src/catalogo.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''            obr(
                "tabelas",
                "lista",''','''            obr(
                "tabelas",
                "array",''')
io.open(p,'w',encoding='utf-8').write(s)
