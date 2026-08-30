# Permissions and clippy
# 28/08 15:28

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "pivotar" | "pivot" => Atividade::Ler,'''
b='''            "pivotar" | "pivot" => Atividade::Ler,
            // Junção e união leem duas ou mais tabelas da MESMA base, e o
            // poder de ler vale por base -- entao ler e o suficiente, e a
            // operacao confere de novo antes de abrir a segunda tabela.
            "juntar" | "join" | "unir" | "union" => Atividade::Ler,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            "sequencias",
            "sistema",'''
b='''            "sequencias",
            "juntar",
            "unir",
            "sistema",'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
