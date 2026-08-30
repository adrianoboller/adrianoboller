# Make response building testable
# 27/08 19:37

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod config;\npub mod servidor;","pub mod config;\npub mod http;\npub mod servidor;")
open(p,'w').write(s)
