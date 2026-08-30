# Register email module and test
# 28/08 14:22

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
if 'pub mod email;' not in s:
    s=s.replace('pub mod config;','pub mod config;\npub mod email;',1)
open(p,'w').write(s)
