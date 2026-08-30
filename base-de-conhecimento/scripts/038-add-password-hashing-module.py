# Add password hashing module
# 27/08 19:00

p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod schema;\npub mod types;","pub mod schema;\npub mod senha;\npub mod types;")
open(p,'w').write(s)
