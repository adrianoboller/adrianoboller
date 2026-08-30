# Fix fixtures and rerun
# 28/08 18:29

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''        // Tres declaradas mais a coluna de sistema, que entra sozinha no fim.
        assert_eq!(e.colunas().len(), 4);
        assert_eq!(e.coluna_softdeleted(), Some(3));''',
'''        // Tres declaradas mais as DUAS de sistema, que entram sozinhas no fim.
        assert_eq!(e.colunas().len(), 5);
        assert_eq!(e.coluna_softdeleted(), Some(3));
        assert_eq!(e.coluna_rownum(), Some(4));''',1)
io.open(p,'w',encoding='utf-8').write(s)
