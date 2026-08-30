# Clean warning and run full tests
# 28/08 17:33

import io
p='crates/phxsql-store/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub use table::{Linha, Relatorio, Table};","pub use table::{Linha, Relatorio, Table, Visao};",1)
io.open(p,'w',encoding='utf-8').write(s)
