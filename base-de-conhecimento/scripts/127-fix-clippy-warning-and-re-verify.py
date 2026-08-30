# Fix clippy warning and re-verify
# 27/08 20:29

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('.map(|i| esquema.colunas()[*i].ty.clone())','.map(|i| esquema.colunas()[*i].ty)')
open(p,'w').write(s)
