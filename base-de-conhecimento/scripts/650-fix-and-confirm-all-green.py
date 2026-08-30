# Fix and confirm all green
# 28/08 18:31

import io
p='crates/phxsql-store/tests/tabela.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''    // Sete declaradas mais a coluna de sistema, que atravessou o disco.
    assert_eq!(t.esquema().colunas().len(), 8);
    assert_eq!(t.esquema().coluna_softdeleted(), Some(7));''',
'''    // Sete declaradas mais as duas de sistema, que atravessaram o disco.
    assert_eq!(t.esquema().colunas().len(), 9);
    assert_eq!(t.esquema().coluna_softdeleted(), Some(7));
    assert_eq!(t.esquema().coluna_rownum(), Some(8));''',1)
io.open(p,'w',encoding='utf-8').write(s)
