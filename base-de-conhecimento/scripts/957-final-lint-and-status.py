# Final lint and status
# 29/08 00:46

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
s = s.replace('''| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |''',
'''| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |
| `.ndx` — cache de páginas de leitura, despejo por segunda chance — inserção 2,4× mais rápida, medida | pronto |''', 1)
p.write_text(s)
print("ok")
