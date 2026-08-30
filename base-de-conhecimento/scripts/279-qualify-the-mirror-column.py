# Qualify the mirror column
# 28/08 11:05

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
trocas = [
 ('<tr><td class="dado">Editar conteúdo</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td>linha a linha, pelo diário</td></tr>',
  '<tr><td class="dado">Editar conteúdo</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">se ligado</td><td>linha a linha, pelo diário</td></tr>'),
 ('<tr><td class="dado">Duplicar</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td>excluindo a cópia</td></tr>',
  '<tr><td class="dado">Duplicar</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">se houver</td><td>excluindo a cópia</td></tr>'),
 ('<tr><td class="dado">Reparar tabela</td><td class="num">grava</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">grava</td><td>não — repara em cima</td></tr>',
  '<tr><td class="dado">Reparar tabela</td><td class="num">grava</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">exige</td><td>não — repara em cima</td></tr>'),
 ('<tr><td class="dado">Excluir</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td><strong>não</strong></td></tr>',
  '<tr><td class="dado">Excluir</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td><strong>não</strong></td></tr>'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:60]
    s = s.replace(v, n)
v = '''<p>Três coisas que a linha do <em>Excluir</em> obriga.'''
n = '''<p><em>Se ligado</em> quer dizer o espelho do <code>config.json</code>: o
  <code>.bkp</code> só existe quando <code>espelho</code> está ligado, e a linha
  do <em>Reparar tabela</em> <strong>exige</strong> que exista — sem espelho não
  há segunda cópia para conferir contra.</p>

  <p>Três coisas que a linha do <em>Excluir</em> obriga.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
