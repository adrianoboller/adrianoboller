# Move the loader to core
# 28/08 19:28

import io
p='crates/phxsql-server/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub mod importar;\n","",1)
io.open(p,'w',encoding='utf-8').write(s)
