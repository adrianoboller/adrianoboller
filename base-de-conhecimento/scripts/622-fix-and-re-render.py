# Fix and re-render
# 28/08 18:10

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
velho='''          <text x="40" y="440" font-size="11.5">O <code>.trash</code> guarda o <tspan font-weight="600">conteúdo</tspan> dos anexos, não os ponteiros — os blocos que ele apontaria acabaram de ser liberados.</text>'''
novo='''          <text x="40" y="440" font-size="11.5">O .trash guarda o <tspan font-weight="600">conteúdo</tspan> dos anexos, não os ponteiros — os blocos que ele apontaria acabaram de ser liberados.</text>'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
