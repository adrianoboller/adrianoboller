# Add challenge-response authentication
# 27/08 19:23

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod datahora;\npub mod error;","pub mod datahora;\npub mod desafio;\npub mod error;")
open(p,'w').write(s)
