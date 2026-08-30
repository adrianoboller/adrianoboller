# Make the script own the cover version badge
# 28/08 20:48

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
antigo = '  <div class="selo">Dossiê técnico · versão 0.11.0</div>'
novo = '''  <!-- selo:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->
  <div class="selo">Dossiê técnico · versão 0.15.0</div>
  <!-- selo:fim -->'''
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

p = pathlib.Path("docs/dossie/numeros-do-projeto.py")
s = p.read_text()
s = s.replace('''ABRE_RODAPE = "<!-- rodape:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA_RODAPE = "<!-- rodape:fim -->"''',
'''ABRE_RODAPE = "<!-- rodape:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA_RODAPE = "<!-- rodape:fim -->"
# O selo da capa tambem: ele ficou quatro lancamentos dizendo 0.11.0, que e
# exatamente o erro que este script existe para nao deixar acontecer.
ABRE_SELO = "<!-- selo:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA_SELO = "<!-- selo:fim -->"''')
p.write_text(s)
