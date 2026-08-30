# Fix assertion and rerun
# 28/08 17:37

import io
p='crates/phxsql-store/tests/tabela.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    assert_eq!(t.esquema().colunas().len(), 7);'''
novo='''    // Sete declaradas mais a coluna de sistema, que atravessou o disco.
    assert_eq!(t.esquema().colunas().len(), 8);
    assert_eq!(t.esquema().coluna_softdeleted(), Some(7));'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
