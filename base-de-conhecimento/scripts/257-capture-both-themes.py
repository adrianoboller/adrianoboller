# Capture both themes
# 28/08 10:47

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''    { rot:"Gerir as tabelas deste banco", ico:"▦", tecla:"Alt+5", faz:gerirTabelasAtual },'''
n = '''    { rot:"Gerir as tabelas",     ico:"▦", tecla:"Alt+5", faz:gerirTabelasAtual },'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''    { rot:"Nova tabela…",                 ico:"✚", faz:() => telaNovaTabela(databaseCorrente()) },'''
n = '''    { rot:"Nova tabela…",         ico:"✚", faz:() => telaNovaTabela(databaseCorrente()) },'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
