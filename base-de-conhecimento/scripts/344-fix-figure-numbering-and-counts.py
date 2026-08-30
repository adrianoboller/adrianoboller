# Fix figure numbering and counts
# 28/08 11:52

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md'); s = p.read_text()
v = '| ☑️ | 15 | Organograma, fluxograma e dossiê | 19 seções, 16 figuras, tudo em SVG à mão |'
n = '| ☑️ | 15 | Organograma, fluxograma e dossiê | $SEC seções, $FIG figuras, tudo em SVG à mão |'
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('PENDENCIAS: contagem do dossie')
