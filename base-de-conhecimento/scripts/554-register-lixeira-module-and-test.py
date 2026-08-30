# Register lixeira module and test
# 28/08 17:29

import io
p='crates/phxsql-store/src/lib.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("pub mod log;","pub mod lixeira;\npub mod log;",1)
s=s.replace("pub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};",
            "pub use lixeira::{Descartada, LixeiraFile, EXT_TRASH, MAGIC_LIXEIRA};\npub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};",1)
io.open(p,'w',encoding='utf-8').write(s)
