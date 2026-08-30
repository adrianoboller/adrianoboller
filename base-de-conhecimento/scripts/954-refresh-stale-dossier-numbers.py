# Refresh stale dossier numbers
# 29/08 00:44

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()

s = s.replace('''  tudo uma vez só: <strong>2.715 → 25.985 linhas/s, 9,6&#215;</strong>, medido com
  20.000 linhas pela rede contra o mesmo trabalho linha a linha.</p>''',
'''  tudo uma vez só: <strong>2.609 → 37.021 linhas/s, 14,2&#215;</strong>, medido com
  20.000 linhas pela rede contra o mesmo trabalho linha a linha, por
  <code>bancada/carga/medir.py</code>.</p>''')
s = s.replace('''  tudo uma vez só: <strong>2.715 → 25.985 linhas/s, 9,6×</strong>, medido com
  20.000 linhas pela rede contra o mesmo trabalho linha a linha.</p>''',
'''  tudo uma vez só: <strong>2.609 → 37.021 linhas/s, 14,2×</strong>, medido com
  20.000 linhas pela rede contra o mesmo trabalho linha a linha, por
  <code>bancada/carga/medir.py</code>.</p>''')

s = s.replace('''  índice —, e a inserção já era o caminho mais caro do motor, com 65% do tempo na
  manutenção do <code>.ndx</code>. O ganho é de tudo que <em>acontecia por
  linha</em> e passou a acontecer uma vez.</p>''',
'''  índice —, e a inserção é o caminho mais caro do motor, com <strong>63,6% do
  tempo na manutenção do <code>.ndx</code></strong>. O ganho é de tudo que
  <em>acontecia por linha</em> e passou a acontecer uma vez — e o cache de páginas
  da 0.17.0 somou por cima, porque um lote inteiro entra numa única abertura de
  tabela e reaproveita as páginas quentes do índice.</p>''')

s = s.replace('''    e a mesma inserção está <strong>7,7&#215;</strong> atrás — 2,8&#215; mais
    rápida no mesmo trabalho, mesma máquina.</p>''',
'''    e a mesma inserção ficou <strong>7,7&#215;</strong> atrás — 2,8&#215; mais
    rápida no mesmo trabalho, mesma máquina. A quarta, depois do cache de páginas,
    trouxe a inserção para <strong>2,6&#215;</strong> atrás.</p>''')

s = s.replace('''  <p>A bancada dizia que a inserção é 7,7× mais lenta que a do MySQL(R), e o''',
'''  <p>A bancada da época dizia que a inserção era 7,7× mais lenta que a do
  MySQL(R), e o''')
p.write_text(s)
print("ok")
