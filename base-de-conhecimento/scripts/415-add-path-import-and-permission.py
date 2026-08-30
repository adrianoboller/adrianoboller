# Add path import and permission
# 28/08 14:24

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "painel" => Atividade::Ler,'''
b='''            "painel" => Atividade::Ler,
            // Ja o monitor da MAQUINA pede administrar. Nome de placa de rede,
            // nome de disco e ponto de montagem descrevem a infraestrutura, e
            // nao o dado -- quem so le uma tabela nao ganha nada com isso e o
            // atacante ganha o mapa.
            "sistema" => Atividade::Administrar,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
