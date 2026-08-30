# Test joins
# 28/08 17:34

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("    let sistema = phxsql_core::schema::COLUNA_SOFTDELETED;\n","    let sistema = COLUNA_SOFTDELETED;\n",1)
io.open(p,'w',encoding='utf-8').write(s)
