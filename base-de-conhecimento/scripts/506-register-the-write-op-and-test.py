# Register the write op and test
# 28/08 16:32

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            "ajustar_sequencia",
            "dblink_salvar",
            "dblink_excluir",
        ] {'''
b='''            "ajustar_sequencia",
            "dblink_salvar",
            "dblink_excluir",
            // Derrubar conexao alheia nao e leitura: um servidor somente
            // leitura nao deve poder interromper o trabalho de ninguem.
            "encerrar_sessao",
        ] {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
