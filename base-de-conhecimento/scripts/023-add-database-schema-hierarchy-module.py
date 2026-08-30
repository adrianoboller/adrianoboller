# Add database/schema hierarchy module
# 27/08 18:30

p='crates/phxsql-store/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod blob;\npub mod log;","pub mod blob;\npub mod catalogo;\npub mod log;")
s=s.replace("pub use blob::{BlobFile, EstatisticaBlob, MAGIC_BIN, MAGIC_MEMO};","pub use blob::{BlobFile, EstatisticaBlob, MAGIC_BIN, MAGIC_MEMO};\npub use catalogo::{qualificar, separar_qualificado, Database, Instancia};")
open(p,'w').write(s)
