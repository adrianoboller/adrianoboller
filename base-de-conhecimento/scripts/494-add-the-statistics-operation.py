# Add the statistics operation
# 28/08 16:27

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {'''
b='''            // As estatisticas resumem o log de acessos, que ja exige
            // administrar: quem ve quanto cada usuario pediu ve o movimento
            // dos outros.
            "estatisticas" | "estatisticas_uso" => Atividade::Administrar,
            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
