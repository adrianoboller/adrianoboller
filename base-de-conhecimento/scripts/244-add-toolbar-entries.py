# Add toolbar entries
# 28/08 10:35

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

# --------------------------------------------------------------- o icone
v = '''  grade: `<rect x="3" y="4" width="18" height="16" rx="2"'''
n = '''  tabelas: `<rect x="3" y="4" width="13" height="12" rx="1.6" fill="none" stroke-width="1.5"/><path d="M3 8h13M8 8v8" stroke-width="1.3"/><path d="M8 8V4M19 8v12H8" fill="none" stroke-width="1.5" stroke-linecap="round"/>`,
  grade: `<rect x="3" y="4" width="18" height="16" rx="2"'''
assert s.count(v) == 1
s = s.replace(v, n)

# ---------------------------------------------------------- a ferramenta
v = '''  { ico:"grade",    rot:"View DB",    cor:"var(--bin)",    faz:viewDatabaseAtual },'''
n = '''  { ico:"grade",    rot:"View DB",    cor:"var(--bin)",    faz:viewDatabaseAtual },
  { ico:"tabelas",  rot:"Tabelas",    cor:"var(--reg)",    faz:gerirTabelasAtual },'''
assert s.count(v) == 1
s = s.replace(v, n)

# a de Transacoes deixa de ser um botao apagado: agora tem tela
v = '''  { ico:"troca",    rot:"Transações", cor:"var(--log)",    faz:null,
    falta:"Não existem. Hoje a inserção desfaz o que gravou se um índice falhar, "
        + "mas não há journal nem commit/rollback de várias operações. É o furo "
        + "que o uso como livro-razão exigiria fechar primeiro." },'''
n = '''  { ico:"troca",    rot:"Transações", cor:"var(--log)",    faz:verTransacoes },'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ferramenta Tabelas + Transacoes ligada')
