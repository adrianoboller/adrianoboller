# Document cache_paginas in MANUAL and PENDENCIAS
# 29/08 00:35

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
s = s.replace('''| ☑️ | 79 | **Seção de cache, memória, CPU, threads e usuários no `config.json`** | seção `recursos`, com sete ajustes. `cache_paginas` e `memoria_max_mb` são lidos e mostrados mas **ainda não impostos** — o buffer pool é o trabalho seguinte |''',
'''| ☑️ | 79 | **Seção de cache, memória, CPU, threads e usuários no `config.json`** | seção `recursos`, com sete ajustes. `cache_paginas` passou a **valer de verdade** na 0.17.0 — era lido e mostrado sem nada por trás, e agora é o teto do cache de páginas do `.ndx`. `memoria_max_mb` continua lido e não imposto |''', 1)
p.write_text(s)
