# Add copiar_tabela to permissions
# 28/08 11:25

import pathlib
p = pathlib.Path('crates/phxsql-server/src/usuarios.rs')
s = p.read_text()
v = '''            "criar_database" | "criar_schema" | "criar_tabela" | "duplicar_tabela" => {
                Atividade::Criar
            }'''
n = '''            "criar_database" | "criar_schema" | "criar_tabela" | "duplicar_tabela"
            | "copiar_tabela" => Atividade::Criar,'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''            "ler" | "varrer" | "buscar"'''
assert s.count(v) >= 0
p.write_text(s)
