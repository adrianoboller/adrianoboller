# Add sessoes and encerrar_sessao
# 28/08 16:31

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "estatisticas" | "estatisticas_uso" => Atividade::Administrar,'''
b='''            "estatisticas" | "estatisticas_uso" => Atividade::Administrar,
            // Ver quem esta conectado e derrubar conexao sao poder de
            // administrador: a lista mostra o login e o IP dos outros, e
            // derrubar interrompe o trabalho alheio.
            "sessoes" | "processlist" | "encerrar_sessao" | "kill" => Atividade::Administrar,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
