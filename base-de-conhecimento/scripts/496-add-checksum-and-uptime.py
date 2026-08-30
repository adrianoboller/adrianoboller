# Add checksum and uptime
# 28/08 16:28

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "sequencias" | "sequences" => Atividade::Ler,'''
b='''            "sequencias" | "sequences" => Atividade::Ler,
            // A soma de verificacao le a tabela inteira e devolve um numero:
            // quem pode ler a tabela pode saber se ela mudou.
            "checksum" | "soma_de_verificacao" => Atividade::Ler,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
