# Add pivotar to the read-only test
# 28/08 13:29

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''            "sistabelas",
            "siscolunas",
        ] {'''
n = '''            "sistabelas",
            "siscolunas",
            "pivotar",
        ] {'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
