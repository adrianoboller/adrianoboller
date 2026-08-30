# Wire permissions and the read-only guard
# 28/08 11:25

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''            "ping", "config", "bancos", "tabelas", "esquema", "ler", "varrer", "buscar", "diario",
            "verificar", "painel", "acessos", "usuarios",
        ] {'''
n = '''            "ping", "config", "bancos", "tabelas", "esquema", "ler", "varrer", "buscar", "diario",
            "verificar", "painel", "acessos", "usuarios", "sistabelas", "siscolunas",
        ] {'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
