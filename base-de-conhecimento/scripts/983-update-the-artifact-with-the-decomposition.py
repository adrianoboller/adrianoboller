# Update the artifact with the decomposition
# 29/08 01:48

import pathlib
p = pathlib.Path("$SC/caminho-da-insercao.html")
s = p.read_text()

# 1) o colchete do fluxograma: os dois deixaram de ser um bloco fechado
alvo = '''      <text x="648" y="800" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">5,4 µs</text>
      <text x="648" y="815" font-size="10.5" fill="currentColor" opacity=".55">31,8%, os dois juntos</text>
      <text x="648" y="831" font-size="10.5" fill="var(--acento)">nunca decompostos</text>'''
novo = '''      <text x="648" y="794" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">5,4 µs</text>
      <text x="648" y="808" font-size="10.5" fill="currentColor" opacity=".55">31,8%, os dois juntos</text>
      <text x="648" y="826" font-size="10.5" fill="currentColor" opacity=".7">.reg  ~4,2</text>
      <text x="648" y="840" font-size="10.5" fill="var(--log)">.log   1,22</text>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# 2) o rodape da barra empilhada
s = s.replace('''<figcaption>A conferência da chave única quase sumiu — era 20,5% antes do cache
  de páginas. Ela é uma descida na árvore que não escreve nada: exatamente o
  trabalho que o cache passou a servir de graça.</figcaption>''',
'''<figcaption>A conferência da chave única quase sumiu — era 20,5% antes do cache
  de páginas. Ela é uma descida na árvore que não escreve nada: exatamente o
  trabalho que o cache passou a servir de graça. E a primeira barra, que era um
  bloco fechado, foi aberta: o <code>.log</code> são 1,22 µs dos 5,4, então o
  <code>.reg</code> sozinho é ~4,2 (§03).</figcaption>''', 1)

# 3) linha nova na tabela do que ja foi tentado
alvo = '''    <tr>
      <td>WAL, MemTable de escrita, group commit</td>'''
novo = '''    <tr>
      <td>Guardar o <code>.log</code> em RAM e gravar no fim</td>
      <td class="num">1,08×</td>
      <td>O diário inteiro custa <strong>1,22 µs, 7,2%</strong> — e um evento perdido <strong>não se reconstrói</strong>: ele é a posição de que a replicação depende. Já a reescrita do cabeçalho, 0,41 µs, sai <em>sem</em> buffer nenhum.</td>
      <td><span class="pino pend">medido, o barato vale</span></td>
    </tr>
    <tr>
      <td>WAL, MemTable de escrita, group commit</td>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
