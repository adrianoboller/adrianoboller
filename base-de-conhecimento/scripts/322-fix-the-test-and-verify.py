# Fix the test and verify
# 28/08 11:25

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''            "painel",
            "acessos",
            "usuarios",
        ] {'''
n = '''            "painel",
            "acessos",
            "usuarios",
            "sistabelas",
            "siscolunas",
        ] {'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
