# Update the disk-touch table
# 28/08 13:11

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
v = '''        <tr><td class="dado">Partições</td><td class="num">lê</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td>não altera</td></tr>'''
n = '''        <tr><td class="dado">Partições</td><td class="num">lê</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td>não altera</td></tr>
        <tr><td class="dado">Configurações e diretivas</td><td class="num">lê</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td>não altera</td></tr>'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        <tr><td class="dado">Duplicar</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">se houver</td><td>excluindo a cópia</td></tr>'''
n = '''        <tr><td class="dado">Duplicar</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">se houver</td><td>excluindo a cópia</td></tr>
        <tr><td class="dado">Copiar / colar</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">se houver</td><td>excluindo a cópia</td></tr>'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''  <p>A conferência de qual arquivo pertence a qual tabela também não é
  ingênua.'''
n = '''  <p><em>Duplicar</em> e <em>copiar/colar</em> fazem a mesma cópia byte a byte; a
  diferença é o alcance. Duplicar fica no mesmo database; colar atravessa para
  outro banco ou schema, e aí a permissão de criar é conferida <strong>no
  destino</strong> — sem isso, quem pode ler um banco e não pode criar no outro
  escreveria onde não devia.</p>

  <p>A conferência de qual arquivo pertence a qual tabela também não é
  ingênua.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('tabela do que cada operacao toca')
