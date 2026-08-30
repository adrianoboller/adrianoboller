# Fix the nested td bug
# 28/08 17:48

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
antes = s.count('<td class="dado">${celulaValor(')
s=s.replace('`<td class="dado">${celulaValor(l[c])}</td>`','celulaValor(l[c])')
io.open(p,'w',encoding='utf-8').write(s)
print('trocadas:', antes)
