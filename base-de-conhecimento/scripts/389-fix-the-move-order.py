# Fix the move order
# 28/08 13:49

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''            config,
            janela: Janela::nova(&config.recursos),'''
n = '''            janela: Janela::nova(&config.recursos),
            config,'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
