# Add Alt+5 shortcut
# 28/08 10:36

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''    if (ev.altKey && ev.key === "4") { ev.preventDefault(); viewDatabaseAtual(); }'''
n = '''    if (ev.altKey && ev.key === "4") { ev.preventDefault(); viewDatabaseAtual(); return; }
    if (ev.altKey && ev.key === "5") { ev.preventDefault(); gerirTabelasAtual(); }'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('Alt+5')
