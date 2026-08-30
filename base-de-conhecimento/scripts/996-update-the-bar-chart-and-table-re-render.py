# Update the bar chart and table, re-render
# 29/08 02:02

import pathlib
p = pathlib.Path("$SC/caminho-da-insercao.html")
s = p.read_text()

# ---- a barra empilhada, com as proporcoes refeitas
alvo = '''      <!-- .reg + .log : 5,4 de 17,0 -->
      <rect x="40" y="74" width="257" height="42" rx="4" fill="var(--reg)" fill-opacity=".88"/>
      <text x="168" y="100" text-anchor="middle" font-size="13" fill="var(--papel)" font-weight="600">.reg + .log</text>
      <text x="168" y="62" text-anchor="middle" font-size="13" fill="var(--reg)" font-family="IBM Plex Mono, monospace" font-weight="600">5,4 µs</text>
      <text x="168" y="46" text-anchor="middle" font-size="10.5" fill="currentColor" opacity=".55">31,8%</text>

      <!-- 1o indice : 5,4 -->
      <rect x="301" y="74" width="257" height="42" rx="4" fill="var(--ndx)" fill-opacity=".88"/>
      <text x="429" y="100" text-anchor="middle" font-size="13" fill="var(--papel)" font-weight="600">1º índice</text>
      <text x="429" y="62" text-anchor="middle" font-size="13" fill="var(--ndx)" font-family="IBM Plex Mono, monospace" font-weight="600">5,4 µs</text>
      <text x="429" y="46" text-anchor="middle" font-size="10.5" fill="currentColor" opacity=".55">31,8%</text>

      <!-- conferir a unica : 0,7 -->
      <rect x="562" y="74" width="33" height="42" rx="4" fill="var(--ndx)" fill-opacity=".42"/>
      <text x="578" y="62" text-anchor="middle" font-size="11" fill="var(--ndx)" font-family="IBM Plex Mono, monospace">0,7</text>

      <!-- 2o indice : 5,5 -->
      <rect x="599" y="74" width="261" height="42" rx="4" fill="var(--ndx)" fill-opacity=".88"/>
      <text x="729" y="100" text-anchor="middle" font-size="13" fill="var(--papel)" font-weight="600">2º índice</text>
      <text x="729" y="62" text-anchor="middle" font-size="13" fill="var(--ndx)" font-family="IBM Plex Mono, monospace" font-weight="600">5,5 µs</text>
      <text x="729" y="46" text-anchor="middle" font-size="10.5" fill="currentColor" opacity=".55">32,4%</text>'''
novo = '''      <!-- .reg + .log : 4,8 de 15,9 -->
      <rect x="40" y="74" width="244" height="42" rx="4" fill="var(--reg)" fill-opacity=".88"/>
      <text x="162" y="100" text-anchor="middle" font-size="13" fill="var(--papel)" font-weight="600">.reg + .log</text>
      <text x="162" y="62" text-anchor="middle" font-size="13" fill="var(--reg)" font-family="IBM Plex Mono, monospace" font-weight="600">4,8 µs</text>
      <text x="162" y="46" text-anchor="middle" font-size="10.5" fill="currentColor" opacity=".55">30,3%</text>

      <!-- 1o indice : 5,4 -->
      <rect x="288" y="74" width="274" height="42" rx="4" fill="var(--ndx)" fill-opacity=".88"/>
      <text x="425" y="100" text-anchor="middle" font-size="13" fill="var(--papel)" font-weight="600">1º índice</text>
      <text x="425" y="62" text-anchor="middle" font-size="13" fill="var(--ndx)" font-family="IBM Plex Mono, monospace" font-weight="600">5,4 µs</text>
      <text x="425" y="46" text-anchor="middle" font-size="10.5" fill="currentColor" opacity=".55">33,9%</text>

      <!-- conferir a unica : 0,3 -->
      <rect x="566" y="74" width="15" height="42" rx="3" fill="var(--ndx)" fill-opacity=".42"/>
      <text x="573" y="62" text-anchor="middle" font-size="11" fill="var(--ndx)" font-family="IBM Plex Mono, monospace">0,3</text>

      <!-- 2o indice : 5,4 -->
      <rect x="585" y="74" width="275" height="42" rx="4" fill="var(--ndx)" fill-opacity=".88"/>
      <text x="722" y="100" text-anchor="middle" font-size="13" fill="var(--papel)" font-weight="600">2º índice</text>
      <text x="722" y="62" text-anchor="middle" font-size="13" fill="var(--ndx)" font-family="IBM Plex Mono, monospace" font-weight="600">5,4 µs</text>
      <text x="722" y="46" text-anchor="middle" font-size="10.5" fill="currentColor" opacity=".55">34,0%</text>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

pares = [
 ('<text x="860" y="138" text-anchor="end" font-size="10" fill="currentColor" opacity=".5" font-family="IBM Plex Mono, monospace">17,0 µs</text>',
  '<text x="860" y="138" text-anchor="end" font-size="10" fill="currentColor" opacity=".5" font-family="IBM Plex Mono, monospace">15,9 µs</text>'),
 ('<path d="M301 156 L301 166 L860 166 L860 156" fill="none" stroke="var(--ndx)" opacity=".7"/>',
  '<path d="M288 156 L288 166 L860 166 L860 156" fill="none" stroke="var(--ndx)" opacity=".7"/>'),
 ('<text x="580" y="188" text-anchor="middle" font-size="13.5" fill="var(--ndx)" font-weight="600">o índice é 64,2% de uma inserção — e cada índice novo cobra outra vez</text>',
  '<text x="574" y="188" text-anchor="middle" font-size="13.5" fill="var(--ndx)" font-weight="600">o índice é 69,8% de uma inserção — e cada índice novo cobra outra vez</text>'),
 ('<text x="168" y="188" text-anchor="middle" font-size="11.5" fill="currentColor" opacity=".6">a linha em si</text>',
  '<text x="162" y="188" text-anchor="middle" font-size="11.5" fill="currentColor" opacity=".6">a linha em si</text>'),
 ('= 4,8 µs, ou 28% de toda a inserção',
  '= 4,8 µs, ou 30% de toda a inserção'),
 ('CRC: 25,4 µs de 44,4, ou 57% de uma inserção. Tirar o CRC da leitura foi o que\n  comprou 2,40×; tirar o da gravação é o que sobrou.',
  'CRC: 25,4 µs de 44,4, ou 57% de uma inserção. Tirar o CRC da leitura foi o que\n  comprou 2,40×; tirar o da gravação é o que sobrou — e hoje ele é o maior\n  pedaço isolado do caminho.'),
]
for a,b in pares:
    assert s.count(a) == 1, a[:60]
    s = s.replace(a,b,1)

# ---- a linha da tabela sobre o log: virou feito
alvo = '''    <tr>
      <td>Guardar o <code>.log</code> em RAM e gravar no fim</td>
      <td class="num">1,08×</td>
      <td>O diário inteiro custa <strong>1,22 µs, 7,2%</strong> — e um evento perdido <strong>não se reconstrói</strong>: ele é a posição de que a replicação depende. Já a reescrita do cabeçalho, 0,41 µs, sai <em>sem</em> buffer nenhum.</td>
      <td><span class="pino pend">medido, o barato vale</span></td>
    </tr>'''
novo = '''    <tr>
      <td><strong>Cabeçalho do <code>.log</code> no <code>sincronizar</code></strong></td>
      <td class="num">1,06×</td>
      <td>O diário fazia <strong>duas escritas por evento</strong>: os 44 bytes do evento, e 64 de cabeçalho só para o contador. O evento continua indo na hora; o contador foi para o <code>sincronizar</code>, e <code>abrir</code> cura varrendo pelo CRC. <strong>1,22 → 0,67 µs por evento.</strong></td>
      <td><span class="pino ok">feito</span></td>
    </tr>
    <tr>
      <td>Guardar os <em>eventos</em> do <code>.log</code> em RAM</td>
      <td class="num">1,04×</td>
      <td>Compraria os 0,67 µs restantes — e um evento perdido <strong>não se reconstrói</strong>: ele é a história, e é a posição de que a replicação depende. Índice perdido volta com <code>reindexar</code>; evento, não.</td>
      <td><span class="pino nao">não vale a troca</span></td>
    </tr>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# figcaption da barra
s = s.replace('''bloco fechado, foi aberta: o <code>.log</code> são 1,22 µs dos 5,4, então o
  <code>.reg</code> sozinho é ~4,2 (§03).</figcaption>''',
'''bloco fechado, foi aberta — e encolheu: o <code>.log</code> são 0,67 µs dos 4,8,
  então o <code>.reg</code> sozinho é ~4,1 (§03).</figcaption>''')
p.write_text(s)
print("ok")
