# Fix the other varrer consumer
# 28/08 18:35

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''      <span class="conta">${r.devolvidas} de ${r.total} linhas · ${esc(r.ordem)}</span>''',
            '''      <span class="conta">${fmt(r.devolvidas)} de ${fmt(r.registros)} linhas · ${esc(r.ordem)}</span>''',1)
io.open(p,'w',encoding='utf-8').write(s)
