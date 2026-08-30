# Add inserir_lote to the Table
# 28/08 19:18

import io
p='crates/phxsql-store/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub use table::{Linha, Relatorio, Table, Visao};","pub use table::{Linha, Lote, Relatorio, Table, Visao};",1)
io.open(p,'w',encoding='utf-8').write(s)
