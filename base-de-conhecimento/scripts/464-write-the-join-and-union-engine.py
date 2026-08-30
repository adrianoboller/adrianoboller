# Write the join and union engine
# 28/08 15:25

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
if 'pub mod juncao;' not in s:
    s=s.replace('pub mod http;','pub mod http;\npub mod juncao;',1)
open(p,'w').write(s)
