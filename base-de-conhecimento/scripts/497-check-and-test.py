# Check and test
# 28/08 16:28

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            "sequencias",
            "juntar",
            "unir",'''
b='''            "sequencias",
            "juntar",
            "unir",
            "estatisticas",
            "checksum",'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
