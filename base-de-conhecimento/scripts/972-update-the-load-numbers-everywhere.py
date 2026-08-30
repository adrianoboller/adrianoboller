# Update the load numbers everywhere
# 29/08 01:13

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
s = s.replace('''| Inserção pela rede, linha a linha vs. lote | 2.609/s | 37.021/s | **14,2×** |
| Inserção local, 2 índices (`onde-doi`) | 22.516/s | 53.988/s | **2,40×** |''',
'''| Inserção pela rede, linha a linha vs. lote | 2.659/s | 39.287/s | **14,8×** |
| Inserção local, 2 índices (`onde-doi`) | 22.516/s | 58.767/s | **2,61×** |''')
s = s.replace('''(2.715 e 2.609), que é o controle; o lote subiu de 25.985 para 37.021 por causa
do cache de páginas.''','''(2.715 e 2.659), que é o controle; o lote subiu de 25.985 para 39.287 por causa
do cache de páginas e do cabeçalho enxuto.''')
p.write_text(s)
