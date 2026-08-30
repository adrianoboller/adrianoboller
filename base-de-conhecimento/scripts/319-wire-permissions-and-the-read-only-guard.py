# Wire permissions and the read-only guard
# 28/08 11:25

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''    "criar_tabela",
    "excluir_tabela",
    "duplicar_tabela",
];'''
n = '''    "criar_tabela",
    "excluir_tabela",
    "duplicar_tabela",
    "copiar_tabela",
];'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
