# Use the schema's columns
# 28/08 11:17

import pathlib
p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''                let i = colunas
                    .iter()
                    .position(|c| c.nome == coluna)'''
n = '''                let i = esquema
                    .colunas()
                    .iter()
                    .position(|c| c.nome == coluna)'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
