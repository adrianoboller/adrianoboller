# Write the connection registry
# 28/08 16:30

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
if 'pub mod ligacoes;' not in s:
    s=s.replace('pub mod juncao;','pub mod juncao;\npub mod ligacoes;',1)
open(p,'w').write(s)
