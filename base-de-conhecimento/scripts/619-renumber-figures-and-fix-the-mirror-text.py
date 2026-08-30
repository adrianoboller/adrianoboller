# Renumber figures and fix the mirror text
# 28/08 18:08

import io, re
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()

# 1. Renumera as figuras 8..23 -> 9..24, de tras para a frente.
for n in range(23, 7, -1):
    velho = f"<b>Figura {n}.</b>"
    novo  = f"<b>Figura {n+1}.</b>"
    assert velho in s, velho
    s = s.replace(velho, novo, 1)

# 2. Corrige o rodape da figura 7 e o texto do espelho.
s = s.replace(
  '''<text x="16" y="434" font-size="10.5" opacity=".55">Excluir tira as chaves, marca os blocos como mortos e marca o slot como livre — sem nunca reaproveitá-lo.</text>''',
  '''<text x="16" y="434" font-size="10.5" opacity=".55">Excluir tem dois caminhos, e o padrão é o reversível — a figura seguinte mostra os dois.</text>''', 1)

s = s.replace('<h3>O sexto arquivo: o espelho <code>.bkp</code></h3>',
              '<h3>O oitavo arquivo: o espelho <code>.bkp</code></h3>', 1)
s = s.replace('''  <p>A tabela é <strong>cinco</strong> arquivos, e um sexto opcional. O''',
              '''  <p>A tabela é <strong>sete</strong> arquivos, e um oitavo opcional. O''', 1)
io.open(p,'w',encoding='utf-8').write(s)
print('renumerado')
