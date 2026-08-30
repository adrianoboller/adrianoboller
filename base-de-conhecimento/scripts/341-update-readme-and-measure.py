# Update README and measure
# 28/08 11:50

import pathlib
p = pathlib.Path('README.md'); s = p.read_text()
trocas = [
 ('O motor de armazenamento está completo e testado: **339 testes**, sem nenhuma',
  'O motor de armazenamento está completo e testado: **355 testes**, sem nenhuma'),
 ('| Paginação em volumes `_001`, `_002`, … com abertura preguiçosa | pronto |',
  '| Paginação em volumes `_001`, `_002`, … com abertura preguiçosa | pronto |\n'
  '| Partição por período — mensal, bimestral, semestral, anual | pronto |\n'
  '| Metadados de campo: id estável, caption, descrição e máscara PICTURE | pronto |\n'
  '| Chave primária declarada, com marca de composta derivada dos índices | pronto |'),
 ('| Gestão de tabelas — criar, duplicar, reparar, ver partições e excluir | pronto |',
  '| Gestão de tabelas — criar, duplicar, reparar, ver partições e excluir | pronto |\n'
  '| Copiar e colar tabela entre bancos e schemas | pronto |\n'
  '| SysTables e SysColumns — catálogo e dicionário de dados | pronto |\n'
  '| Gerir banco: configurações, diretivas de acesso, conexões, backup | pronto |\n'
  '| Editor de menu — troca o nome exibido de cada item | pronto |'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:50]
    s = s.replace(v, n)
p.write_text(s)
print('README')
