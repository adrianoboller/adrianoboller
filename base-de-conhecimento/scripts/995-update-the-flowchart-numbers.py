# Update the flowchart numbers
# 29/08 02:02

import pathlib
p = pathlib.Path("$SC/caminho-da-insercao.html")
s = p.read_text()
pares = [
 # KPI
 ('<div><div class="v">17,0 µs</div><div class="r">por linha, 2 índices</div></div>',
  '<div><div class="v">15,9 µs</div><div class="r">por linha, 2 índices</div></div>'),
 ('<div><div class="v">64,2%</div><div class="r">disso está no .ndx</div></div>',
  '<div><div class="v">69,8%</div><div class="r">disso está no .ndx</div></div>'),
 ('<div><div class="v">2,61×</div><div class="r">ganho nesta versão</div></div>',
  '<div><div class="v">2,79×</div><div class="r">ganho nesta versão</div></div>'),
 # fluxograma: a caixa do ndx
 ('<text x="636" y="716" font-size="19" fill="var(--ndx)" font-weight="700" font-family="IBM Plex Mono, monospace">10,9 µs</text>',
  '<text x="636" y="716" font-size="19" fill="var(--ndx)" font-weight="700" font-family="IBM Plex Mono, monospace">11,1 µs</text>'),
 ('<text x="636" y="736" font-size="11" fill="var(--ndx)">64,2% do total</text>',
  '<text x="636" y="736" font-size="11" fill="var(--ndx)">69,8% do total</text>'),
 # fluxograma: o losango
 ('<text x="636" y="466" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">0,7 µs</text>',
  '<text x="636" y="466" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">0,3 µs</text>'),
 ('<text x="636" y="480" font-size="10.5" fill="currentColor" opacity=".55">4,0% do total</text>',
  '<text x="636" y="480" font-size="10.5" fill="currentColor" opacity=".55">1,9% do total</text>'),
 # fluxograma: o colchete reg+log
 ('<text x="648" y="794" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">5,4 µs</text>',
  '<text x="648" y="794" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">4,8 µs</text>'),
 ('<text x="648" y="808" font-size="10.5" fill="currentColor" opacity=".55">31,8%, os dois juntos</text>',
  '<text x="648" y="808" font-size="10.5" fill="currentColor" opacity=".55">30,3%, os dois juntos</text>'),
 ('<text x="648" y="826" font-size="10.5" fill="currentColor" opacity=".7">.reg  ~4,2</text>',
  '<text x="648" y="826" font-size="10.5" fill="currentColor" opacity=".7">.reg  ~4,1</text>'),
 ('<text x="648" y="840" font-size="10.5" fill="var(--log)">.log   1,22</text>',
  '<text x="648" y="840" font-size="10.5" fill="var(--log)">.log   0,67</text>'),
 # a caixa do .log no fluxograma
 ('<text x="110" y="822" font-size="11.5" fill="currentColor" opacity=".62">44 bytes; com replicação ligada, ~223 — a imagem da linha vai junto</text>',
  '<text x="110" y="822" font-size="11.5" fill="currentColor" opacity=".62">44 bytes, gravados na hora; o contador do cabeçalho vai no sincronizar</text>'),
]
for a,b in pares:
    assert s.count(a) == 1, a[:60]
    s = s.replace(a,b,1)
p.write_text(s)
print("fluxograma ok")
