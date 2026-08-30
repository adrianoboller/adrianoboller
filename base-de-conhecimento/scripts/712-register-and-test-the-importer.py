# Register and test the importer
# 28/08 19:20

import io
p='crates/phxsql-server/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub mod http;","pub mod http;\npub mod importar;",1)
io.open(p,'w',encoding='utf-8').write(s)
