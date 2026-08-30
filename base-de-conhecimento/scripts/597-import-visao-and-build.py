# Import Visao and build
# 28/08 17:49

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("use phxsql_store::table::Table;","use phxsql_store::table::{Table, Visao};",1)
io.open(p,'w',encoding='utf-8').write(s)
