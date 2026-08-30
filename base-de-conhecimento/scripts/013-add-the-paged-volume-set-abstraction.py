# Add the paged volume-set abstraction
# 27/08 18:20

p='crates/phxsql-store/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod table;\nmod util;","pub mod table;\nmod util;\npub mod volume;")
s=s.replace("pub use table::{Linha, Relatorio, Table};","pub use table::{Linha, Relatorio, Table};\npub use volume::Volumes;")
open(p,'w').write(s)
