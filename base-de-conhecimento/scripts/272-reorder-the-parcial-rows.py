# Reorder the parcial rows
# 28/08 11:02

import pathlib, re
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()
linhas = s.split('\n')
# a linha 3 (chave estrangeira) tem de vir depois da 2 (GitHub)
i3 = next(i for i,l in enumerate(linhas) if l.startswith('| 3 | **Chave estrangeira**'))
i2 = next(i for i,l in enumerate(linhas) if l.startswith('| 2 | **Subir o PhxSql'))
assert i3 < i2
linhas[i3], linhas[i2] = linhas[i2], linhas[i3]
p.write_text('\n'.join(linhas))
print('ok')
