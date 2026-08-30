# Add fixed parts and tests
# 28/08 16:55

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
if 'pub mod exportar;' not in s:
    s=s.replace('pub mod email;','pub mod email;\npub mod exportar;',1)
open(p,'w').write(s)
