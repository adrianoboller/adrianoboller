# Update the parcial and planejado sections
# 28/08 11:01

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()

# o parcial nº 3 sai da lista de parciais
v = '''| 3 | **Quantidade de registros e arquivos definida no create table** | a paginação é parâmetro do esquema e funciona; `criar_tabela` existe na biblioteca | não há **op no protocolo nem comando na CLI** para criar tabela. Hoje só se cria escrevendo Rust. Criar *database* pela rede já dá |
'''
assert s.count(v) == 1
s = s.replace(v, '')

# renumera os que ficaram
v = '''| 4 | **Gráficos comparativos** de IO, memória e CPU |'''
n = '''| 3 | **Gráficos comparativos** de IO, memória e CPU |'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''| 5 | **Subir o PhxSql no GitHub** |'''
n = '''| 4 | **Subir o PhxSql no GitHub** |'''
assert s.count(v) == 1
s = s.replace(v, n)

# transacoes agora tem tela -- que diz que nao existe
v = '''| 11 | Transações | — | hoje a inserção desfaz o que gravou se um índice falhar, mas não há journal nem `commit`/`rollback` de várias operações |'''
n = '''| 11 | Transações | tem **tela** (Ferramentas → Gestão de transações), e a tela diz o que existe e o que não existe em vez de fingir | hoje a inserção desfaz o que gravou se um índice falhar, e a trava única serializa as escritas — mas não há journal com a imagem anterior da linha, nem identificador de transação na sessão, nem `commit`/`rollback` de várias operações. É o que o uso como livro-razão exigiria primeiro |'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
