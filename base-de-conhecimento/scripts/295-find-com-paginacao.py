# Find com_paginacao
# 28/08 11:16

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()
v = '''        // So uma chave primaria, e ela e unica.'''
n = '''        // A particao por periodo aponta uma coluna, e ela tem de existir e ser
        // uma data. Conferir aqui e nao na gravacao: um esquema que so quebra
        // na primeira insercao ja nasceu quebrado.
        // So uma chave primaria, e ela e unica.'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
