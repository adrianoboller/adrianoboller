# Add the three ops to the read-only guard
# 28/08 10:54

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''    "reindexar",
    "criar_database",
    "criar_schema",
];'''
n = '''    "reindexar",
    "criar_database",
    "criar_schema",
    "criar_tabela",
    "excluir_tabela",
    "duplicar_tabela",
];'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
