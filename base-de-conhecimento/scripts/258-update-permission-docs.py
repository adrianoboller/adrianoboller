# Update permission docs
# 28/08 10:49

import pathlib
p = pathlib.Path('docs/USUARIOS.md')
s = p.read_text()
v = '''| `criar` | `criar_database`, `criar_schema` |'''
n = '''| `criar` | `criar_database`, `criar_schema`, `criar_tabela`, `duplicar_tabela` |'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''| `administrar` | `acessos`, `ips`, `config`, `usuarios` |'''
n = '''| `administrar` | `acessos`, `ips`, `config`, `usuarios`, `excluir_tabela` |'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
