# Update manual and permission docs
# 28/08 11:48

import pathlib

# ------------------------------------------------------------------ MANUAL
p = pathlib.Path('MANUAL.txt')
s = p.read_text()
v = '''    criar_schema    database, schema
    criar_tabela    database, [schema], tabela, colunas, [indices],
                    [registros_por_arquivo], [digitos], [max_arquivos]
    duplicar_tabela database, tabela, destino
    excluir_tabela  database, tabela, confirmar   confirmar repete o nome'''
n = '''    criar_schema    database, schema
    criar_tabela    database, [schema], tabela, colunas, [indices],
                    [registros_por_arquivo], [digitos], [max_arquivos],
                    [particao], [particao_coluna]
    duplicar_tabela database, tabela, destino
    copiar_tabela   database, tabela, destino_database, destino
    excluir_tabela  database, tabela, confirmar   confirmar repete o nome
    sistabelas      database                 o catalogo de tabelas
    siscolunas      database, [tabela]       o dicionario de dados'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    criar        criar_database, criar_schema, criar_tabela, duplicar_tabela'''
n = '''    criar        criar_database, criar_schema, criar_tabela, duplicar_tabela,
                 copiar_tabela'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    ler          bancos, tabelas, esquema, ler, varrer, buscar'''
n = '''    ler          bancos, tabelas, esquema, ler, varrer, buscar,
                 sistabelas, siscolunas'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('MANUAL: operacoes e permissoes')

# --------------------------------------------------------------- USUARIOS.md
p = pathlib.Path('docs/USUARIOS.md')
s = p.read_text()
v = '''| `ler` | `bancos`, `tabelas`, `esquema`, `ler`, `varrer`, `buscar` |'''
n = '''| `ler` | `bancos`, `tabelas`, `esquema`, `ler`, `varrer`, `buscar`, `sistabelas`, `siscolunas` |'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''| `criar` | `criar_database`, `criar_schema`, `criar_tabela`, `duplicar_tabela` |'''
n = '''| `criar` | `criar_database`, `criar_schema`, `criar_tabela`, `duplicar_tabela`, `copiar_tabela` |'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''> **Por que `excluir_tabela` pede `administrar` e não `excluir`.**'''
n = '''> **`copiar_tabela` confere a permissão no DESTINO.** O portão geral confere
> contra o database do campo `database` — que aqui é a *origem*. Colar exige
> `criar` no banco de destino, conferido à parte: sem isso, quem pode ler um
> banco e não pode criar no outro conseguiria escrever onde não devia.

> **Por que `excluir_tabela` pede `administrar` e não `excluir`.**'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('USUARIOS.md')
