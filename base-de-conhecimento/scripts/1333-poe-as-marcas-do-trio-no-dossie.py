# Poe as marcas do trio no dossie
# 01/09 18:37

from pathlib import Path
p = Path("docs/dossie/dossie-phxsql-0.18.html")
s = p.read_text(encoding="utf-8")

# O trio entra na secao da bancada, que e onde ele pertence -- e assim nao
# renumera as tres secoes seguintes nem a navegacao.
s = s.replace(
    "<h2>A bancada: dez milhões de linhas, medidas</h2>",
    "<h2>A bancada: dez milhões de linhas, e os três motores a um milhão</h2>",
    1,
)
alvo = '''  <p>A bancada inteira está em <code>bancada/</code> — o medidor, os gráficos, o
  registro bruto da carga e o <code>resultados.json</code>. Qualquer número desta
  seção sai de lá; nenhum foi estimado.</p>'''
assert s.count(alvo) == 1
s = s.replace(alvo, '''<!-- trio:inicio (gerado por docs/dossie/trio-de-motores.py) -->
<!-- trio:fim -->

''' + alvo, 1)

# Dois links que apontam para a secao 33 e a chamam de «secao 32» no texto.
s = s.replace('<a href="#s33">seção 32</a>', '<a href="#s33">seção 33</a>')
p.write_text(s, encoding="utf-8")
print("dossie: marcas do trio postas, titulo da bancada atualizado")
