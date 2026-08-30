# Add the .log journal module
# 27/08 18:25

p='crates/phxsql-store/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod blob;\npub mod ndx;","pub mod blob;\npub mod log;\npub mod ndx;")
s=s.replace("pub use ndx::{DescritorIndice, NdxFile, MAGIC_NDX, PAGINA_PADRAO};","pub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};\npub use ndx::{DescritorIndice, NdxFile, MAGIC_NDX, PAGINA_PADRAO};")
open(p,'w').write(s)
