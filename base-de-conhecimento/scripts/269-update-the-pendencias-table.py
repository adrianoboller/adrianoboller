# Update the PENDENCIAS table
# 28/08 11:01

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()

# ------------------------------------------------ #14 sai de parcial
v = '''| ◐ | 14 | **Quantidade de registros e arquivos no create table** | a paginação funciona, mas **não há op no protocolo nem comando na CLI para criar tabela** — só escrevendo Rust |'''
n = '''| ☑️ | 14 | **Quantidade de registros e arquivos no create table** | op `criar_tabela` no protocolo e tela **Nova tabela** com registros por arquivo, dígitos do sufixo e teto de volumes. A CLI ainda não tem o comando |'''
assert s.count(v) == 1
s = s.replace(v, n)

# ------------------------------------------------ #30 e a cobertura medida
v = '''| ☑️ | 30 | **Interface web parecida com o Centro de Controle HFSQL(R)** | árvore, abas, painel, administração, menu, ferramentas e **View Database com edição** — 30 das 32 operações. Fora: `buscar` e `desbloquear` |'''
n = '''| ☑️ | 30 | **Interface web parecida com o Centro de Controle HFSQL(R)** | árvore, abas, painel, administração, menu, ferramentas, **View Database com edição** e **gestão de tabelas** — 30 das 33 operações. Fora: `buscar`, `desbloquear` e `criar_schema`, que acontece sozinho quando a tela cria tabela dentro de um schema |'''
assert s.count(v) == 1
s = s.replace(v, n)

# ------------------------------------------------ a linha nova
v = '''| ☑️ | 64 | Cadê o sol e a lua? | respondida — estavam lá, o recorte da captura é que cortava |
'''
n = '''| ☑️ | 64 | Cadê o sol e a lua? | respondida — estavam lá, o recorte da captura é que cortava |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco: nova, estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar índice e excluir — e **Gestão de transações** no menu de ferramentas | as oito operações funcionam de ponta a ponta; três delas (`criar_tabela`, `duplicar_tabela`, `excluir_tabela`) nasceram aqui, e `criar_schema` — prometido na documentação e nunca despachado — junto |
'''
assert s.count(v) == 1
s = s.replace(v, n)

# ------------------------------------------------ a contagem
v = '''**57 feitos · 3 parciais · 6 planejados**, de 66 pedidos.

Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, e onze correções
de defeito — três delas de perda silenciosa de dado.'''
n = '''**59 feitos · 2 parciais · 6 planejados**, de 67 pedidos.

Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, e catorze correções
de defeito — três delas de perda silenciosa de dado.'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
