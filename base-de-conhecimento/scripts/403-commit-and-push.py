# Commit and push
# 28/08 14:11

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md'); s = p.read_text()
v = '**View Database com edição**, **gestão de tabelas** e **gestão do banco** — 33 das 36 operações.'
n = '**View Database com edição**, **gestão de tabelas** e **gestão do banco** — 36 das 39 operações.'
assert s.count(v) == 1
p.write_text(s.replace(v, n))
