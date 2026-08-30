# Add sessoes and encerrar_sessao
# 28/08 16:31

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            "estatisticas",
            "checksum",'''
b='''            "estatisticas",
            "checksum",
            "sessoes",'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
