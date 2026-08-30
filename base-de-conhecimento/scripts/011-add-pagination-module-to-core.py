# Add pagination module to core
# 27/08 18:18

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod keyenc;\npub mod schema;","pub mod keyenc;\npub mod paginacao;\npub mod schema;")
s=s.replace("pub use value::{ler_inline, escrever_inline, Ponteiro, Value};","pub use paginacao::Paginacao;\npub use value::{escrever_inline, ler_inline, Ponteiro, Value};")
open(p,'w').write(s)
