# Update the MANUAL operations list
# 28/08 11:07

import pathlib
p = pathlib.Path('MANUAL.txt')
s = p.read_text()

v = '''    criar_database  database
    ler             database, tabela, rowid'''
n = '''    criar_database  database
    criar_schema    database, schema
    criar_tabela    database, [schema], tabela, colunas, [indices],
                    [registros_por_arquivo], [digitos], [max_arquivos]
    duplicar_tabela database, tabela, destino
    excluir_tabela  database, tabela, confirmar   confirmar repete o nome
    ler             database, tabela, rowid'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    criar        criar_database, criar_schema
    reindexar    reindexar
    diario       diario
    verificar    verificar
    administrar  acessos, ips, config, usuarios'''
n = '''    criar        criar_database, criar_schema, criar_tabela, duplicar_tabela
    reindexar    reindexar
    diario       diario
    verificar    verificar
    administrar  acessos, ips, config, usuarios, excluir_tabela'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
