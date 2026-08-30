# Register motivo module and test
# 28/08 17:27

import io
p='crates/phxsql-store/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub mod memoria;","pub mod memoria;\npub mod motivo;",1)
s=s.replace("pub use memoria::{Consulta, Filtro, Operador, Ordem, Resultado, TabelaMemoria};",
            "pub use memoria::{Consulta, Filtro, Operador, Ordem, Resultado, TabelaMemoria};\npub use motivo::{Motivo, MotivoFile, EXT_REASON, MAGIC_MOTIVO};",1)
io.open(p,'w',encoding='utf-8').write(s)
