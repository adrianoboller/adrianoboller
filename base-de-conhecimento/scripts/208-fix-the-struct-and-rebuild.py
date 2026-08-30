# Fix the struct and rebuild
# 27/08 21:44

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
s=s.replace('''    criado_em: i64,
    recuperados: 0,
    /// Leituras salvas pelo espelho nesta sessao.
    recuperados: u64,
}''','''    criado_em: i64,
    /// Leituras salvas pelo espelho nesta sessao.
    recuperados: u64,
}''')
open(p,'w').write(s)
