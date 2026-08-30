# Final cleanup and check the dossier numbers
# 29/08 01:13

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
s = s.replace('''  2,40×** —, e a carga em lote pela rede subiu de **25.985 para 37.021
  linhas/s**.''','''  2,40×** —, e a carga em lote pela rede subiu de **25.985 para 39.287
  linhas/s** (com o §2.0 junto).''')
p.write_text(s)
