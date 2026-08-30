# Wire the export operation
# 28/08 16:59

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "checksum" | "soma_de_verificacao" => Atividade::Ler,'''
b='''            "checksum" | "soma_de_verificacao" => Atividade::Ler,
            // Exportar e ler a tabela inteira e levar embora. Nao e mais poder
            // do que `varrer` ja da -- e menos, porque nao altera nada.
            "exportar" | "export" => Atividade::Ler,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
