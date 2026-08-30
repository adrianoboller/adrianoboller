# Fix test constructor, lint and count
# 28/08 18:05

import io
p='crates/phxsql-store/src/lixeira.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''            rowid: 1,
            usuario: 1,
            payload: vec![1, 2, 3, 4],
            externos: vec![(0, b"anexo".to_vec())],
        };''','''            rowid: 1,
            usuario: 1,
            n_externos: 1,
            payload: vec![1, 2, 3, 4],
            externos: vec![(0, b"anexo".to_vec())],
        };''',1)
io.open(p,'w',encoding='utf-8').write(s)
