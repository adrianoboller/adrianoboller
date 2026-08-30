# Add por_letra constructor and test the .pag
# 28/08 18:50

import io
p='crates/phxsql-store/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub mod ndx;","pub mod ndx;\npub mod pag;",1)
s=s.replace("pub use ndx::{DescritorIndice, NdxFile, MAGIC_NDX, PAGINA_PADRAO};",
            "pub use ndx::{DescritorIndice, NdxFile, MAGIC_NDX, PAGINA_PADRAO};\npub use pag::EXT_PAG;",1)
io.open(p,'w',encoding='utf-8').write(s)
