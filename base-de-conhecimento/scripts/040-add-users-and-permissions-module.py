# Add users and permissions module
# 27/08 19:03

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod servidor;\npub mod valores;","pub mod servidor;\npub mod usuarios;\npub mod valores;")
s=s.replace("pub use servidor::Servidor;","pub use servidor::Servidor;\npub use usuarios::{Atividade, Cadastro, Permissoes, Usuario};")
open(p,'w').write(s)
