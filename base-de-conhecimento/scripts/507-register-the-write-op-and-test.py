# Register the write op and test
# 28/08 16:32

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''    "dblink_salvar",
    "dblink_excluir",
];'''
b='''    "dblink_salvar",
    "dblink_excluir",
    "encerrar_sessao",
];'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
