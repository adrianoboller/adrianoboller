# Wire the pivot button and menus
# 28/08 13:32

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

# o icone
v = '''  gerir: `<ellipse cx="12" cy="6"'''
n = '''  pivot: `<rect x="3" y="4" width="18" height="16" rx="1.8" fill="none" stroke-width="1.5"/><path d="M3 9h18M9 4v16" stroke-width="1.4"/><path d="M12 12.5h2.5M12 16h5M15.5 12.5h2.5" stroke-width="1.5" stroke-linecap="round"/>`,
  gerir: `<ellipse cx="12" cy="6"'''
assert s.count(v) == 1
s = s.replace(v, n)

# a ferramenta, na familia roxa da consulta -- pivot e consulta
v = '''  { ico:"consulta", rot:"Query",      cor:"var(--ndx)",    faz:abrirConsulta },'''
n = '''  { ico:"consulta", rot:"Query",      cor:"var(--ndx)",    faz:abrirConsulta },
  { ico:"pivot",    rot:"Pivot",      cor:"var(--ndx)",    faz:() => telaPivot() },'''
assert s.count(v) == 1
s = s.replace(v, n)

# no menu Ferramentas e no menu Banco
v = '''    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    "sep",'''
n = '''    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    { rot:"Tabela dinâmica…",     ico:"▦", tecla:"Alt+7", faz:() => telaPivot() },
    "sep",'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    { rot:"SysColumns",           ico:"☰", faz:() => verSysColumns() },
    "sep",'''
n = '''    { rot:"SysColumns",           ico:"☰", faz:() => verSysColumns() },
    { rot:"Tabela dinâmica…",     ico:"▦", faz:() => telaPivot() },
    "sep",'''
assert s.count(v) == 1
s = s.replace(v, n)

# o atalho
v = '''    if (ev.altKey && ev.key === "6") { ev.preventDefault(); gerirDatabase(); }'''
n = '''    if (ev.altKey && ev.key === "6") { ev.preventDefault(); gerirDatabase(); return; }
    if (ev.altKey && ev.key === "7") { ev.preventDefault(); telaPivot(); }'''
assert s.count(v) == 1
s = s.replace(v, n)

# e na tela de gerir banco
v = '''    { g:"dados", ico:"⧉", rot:"Copiar tabela…",'''
n = '''    { g:"dados", ico:"▦", rot:"Tabela dinâmica…",
      diz:"Cruza uma tabela por dois eixos e soma no servidor — com junção a outras tabelas.",
      faz:() => telaPivot(db) },
    { g:"dados", ico:"⧉", rot:"Copiar tabela…",'''
assert s.count(v) == 1
s = s.replace(v, n)

# o estado do assistente
v = '''              copia:null, rotulos:null };'''
n = '''              copia:null, rotulos:null,
              // O assistente do pivot sobrevive a ir e voltar entre os passos.
              pivot:null };'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('botao, menus e atalho do pivot')
