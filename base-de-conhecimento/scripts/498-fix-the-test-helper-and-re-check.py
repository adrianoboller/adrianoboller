# Fix the test helper and re-check
# 28/08 16:29

p='crates/phxsql-server/src/acesso.rs'
s=open(p).read()
a='''            erro: if ok {
                None
            } else {
                Some("token invalido".into())
            },
        }'''
b='''            erro: if ok {
                None
            } else {
                Some("token invalido".into())
            },
            codigo: if ok { 0 } else { 4001 },
            ..Acesso::default()
        }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
