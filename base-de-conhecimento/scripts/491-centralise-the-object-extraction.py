# Centralise the object extraction
# 28/08 16:26

p='crates/phxsql-server/src/acesso.rs'
s=open(p).read()
a='''pub struct Acesso {'''
b='''#[derive(Default)]
pub struct Acesso {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
