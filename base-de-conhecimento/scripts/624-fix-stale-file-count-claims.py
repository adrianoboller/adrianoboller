# Fix stale file-count claims
# 28/08 18:13

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
trocas = [
 ('cada tabela é um conjunto de cinco arquivos"',
  'cada tabela é um conjunto de sete arquivos"'),
 ('''de fronteiras num <strong>sexto arquivo</strong> quebraria o modelo de cinco.''',
  '''de fronteiras num <strong>arquivo próprio</strong> acrescentaria mais um
  formato ao conjunto, para uma tabelinha que cabe no cabeçalho.'''),
 ('''<tr><td>Estrutura</td><td>colunas, em qual dos cinco arquivos cada uma mora, índices, chaves estrangeiras, paginação</td><td><code>esquema</code></td></tr>''',
  '''<tr><td>Estrutura</td><td>colunas, em qual dos arquivos cada uma mora, índices, chaves estrangeiras, paginação</td><td><code>esquema</code></td></tr>'''),
 ('''conjunto diferente dos cinco arquivos, e é isso que decide o que é reversível
  e o que não é. <em>Reparar índice</em> joga fora um arquivo que sabe
  reconstruir sozinho; <em>Excluir</em> apaga seis e não sabe reconstruir
  nenhum.</p>''',
  '''conjunto diferente dos arquivos da tabela, e é isso que decide o que é
  reversível e o que não é. <em>Reparar índice</em> joga fora um arquivo que
  sabe reconstruir sozinho; <em>Excluir tabela</em> apaga os sete mais o
  espelho, e não sabe reconstruir nenhum.</p>'''),
 ('''  que é <code>fsync</code> em até cinco arquivos. Medindo os dois lados''',
  '''  que é <code>fsync</code> em todos os arquivos da tabela. Medindo os dois lados'''),
]
for velho, novo in trocas:
    assert velho in s, velho[:60]
    s = s.replace(velho, novo, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
