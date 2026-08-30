# Corrigir a chamada e rodar
# 29/08 03:26

import io
p='crates/phxsql-store/tests/ndx.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("""        n.intervalo(0, Some(&chave(10)), Some(&chave(20)), true, true)
            .unwrap()
            .len(),
        11""","""        n.intervalo(0, Some(&chave(10)), Some(&chave(20)))
            .unwrap()
            .len(),
        11""")
io.open(p,'w',encoding='utf-8').write(s)
