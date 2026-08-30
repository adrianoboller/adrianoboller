# Write the DbLink registry
# 28/08 14:44

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
if 'pub mod dblink;' not in s:
    s=s.replace('pub mod config;','pub mod config;\npub mod dblink;',1)
open(p,'w').write(s)
