# Add the bulk insert operation
# 28/08 19:21

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''    "ajustar_sequencia",''','''    "ajustar_sequencia",
    "inserir_lote",''',1)
io.open(p,'w',encoding='utf-8').write(s)
