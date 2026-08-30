# Fix borrow and rerun
# 28/08 17:38

import io
p='crates/phxsql-store/tests/exclusao.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("    assert!(!t.esta_excluida(&t.ler(2).unwrap().unwrap()));",
            "    let viva = t.ler(2).unwrap().unwrap();\n    assert!(!t.esta_excluida(&viva));",1)
io.open(p,'w',encoding='utf-8').write(s)
