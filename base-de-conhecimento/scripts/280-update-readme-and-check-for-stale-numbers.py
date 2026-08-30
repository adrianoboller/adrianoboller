# Update README and check for stale numbers
# 28/08 11:05

import pathlib
p = pathlib.Path('README.md')
s = p.read_text()
trocas = [
 ('O motor de armazenamento está completo e testado: **324 testes**, sem nenhuma',
  'O motor de armazenamento está completo e testado: **339 testes**, sem nenhuma'),
 ('| Barra de menu tradicional — 22 recursos, atalhos e navegação por teclado | pronto |',
  '| Barra de menu tradicional — sete menus, atalhos e navegação por teclado | pronto |'),
 ('| View Database — grade de tabelas, ficha de edição, incluir/salvar/excluir | pronto |',
  '| View Database — grade de tabelas, ficha de edição, incluir/salvar/excluir | pronto |\n'
  '| Gestão de tabelas — criar, duplicar, reparar, ver partições e excluir | pronto |'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:60]
    s = s.replace(v, n)
p.write_text(s)
print('ok')
