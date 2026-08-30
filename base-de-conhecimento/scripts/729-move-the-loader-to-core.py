# Move the loader to core
# 28/08 19:28

import io
p='crates/phxsql-core/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub mod base64;","pub mod base64;\npub mod carga;",1)
io.open(p,'w',encoding='utf-8').write(s)
