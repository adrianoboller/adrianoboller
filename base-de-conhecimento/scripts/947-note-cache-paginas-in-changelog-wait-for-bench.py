# Note cache_paginas in CHANGELOG; wait for bench
# 29/08 00:37

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''### Mudado

- **O erro do protocolo chega inteiro à tela.**'''
novo = '''### Mudado

- **`recursos.cache_paginas` passou a valer.** O campo estava no `config.json`,
  no MANUAL e na tela desde a 0.13.0, e **nenhuma linha de código o lia** — ele
  dizia «páginas do `.ndx` mantidas em memória» quando não havia cache nenhum.
  Agora é o teto do cache, e o padrão baixou de 4.096 para **2.048 páginas
  (8 MiB)**, que é o joelho da curva medida.

- **O erro do protocolo chega inteiro à tela.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
