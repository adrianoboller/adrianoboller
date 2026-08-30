# Fix test assertions
# 28/08 18:33

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
i=s.index('mod testes_exclusao')
cab, corpo = s[:i], s[i:]
corpo = corpo.replace('v.inteiro_ou("total", -1)','v.inteiro_ou("devolvidas", -1)')
io.open(p,'w',encoding='utf-8').write(cab+corpo)
