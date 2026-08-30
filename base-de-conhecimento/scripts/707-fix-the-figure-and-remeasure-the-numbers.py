# Fix the figure and remeasure the numbers
# 28/08 19:11

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
velho='''          <text x="150" y="316" font-size="10.5" opacity=".55">Medido com o exemplo custo-da-pagina, três tamanhos, a mesma página. O cursor não deu tempo mensurável em nenhum.</text>'''
novo='''          <text x="90" y="316" font-size="10" opacity=".55">Medido com o exemplo custo-da-pagina, três tamanhos, a mesma página. O cursor não deu tempo mensurável em nenhum.</text>'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
