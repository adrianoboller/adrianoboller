# Add sistema to read-only op test, fmt and clippy
# 28/08 14:24

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            "painel",
            "acessos",'''
b='''            "painel",
            "sistema",
            "acessos",'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
