# Permissions and op-list tests
# 28/08 14:48

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
a='''            "sistema" => Atividade::Administrar,'''
b='''            "sistema" => Atividade::Administrar,
            // DbLink inteiro exige administrar, inclusive o que so LE do
            // outro banco. Uma ligacao guarda UMA credencial, e quem a usa
            // fala com o outro servidor como aquele usuario -- as permissoes
            // por base do PhxSql nao atravessam. Deixar um leitor navegar por
            // ela seria emprestar o poder de quem a criou.
            op if op.starts_with("dblink") => Atividade::Administrar,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            "copiar_tabela",
            "ajustar_sequencia",
        ] {'''
b='''            "copiar_tabela",
            "ajustar_sequencia",
            "dblink_salvar",
            "dblink_excluir",
        ] {'''
assert a in s; s=s.replace(a,b,1)
a='''            "sequencias",
        ] {
            assert!(
                !OPS_ESCRITA.contains(&op),'''
b='''            "sequencias",
            "sistema",
            "dblink",
            "dblink_testar",
            "dblink_tabelas",
            "dblink_estrutura",
            "dblink_ler",
            // Nao esta na lista de proposito: por ela passa tanto consulta
            // quanto escrita, e a propria operacao confere qual e -- barrando
            // a escrita quando este servidor esta somente-leitura, mas
            // deixando a LEITURA funcionar num espelho.
            "dblink_consultar",
        ] {
            assert!(
                !OPS_ESCRITA.contains(&op),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
