# Add zero-dependency JSON parser
# 27/08 18:38

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod error;\npub mod keyenc;","pub mod error;\npub mod json;\npub mod keyenc;")
s=s.replace("pub use paginacao::Paginacao;","pub use json::Json;\npub use paginacao::Paginacao;")
open(p,'w').write(s)
