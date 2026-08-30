# Fix duplicate and rerun screen tests
# 28/08 11:35

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    { rot:"Editor de menu…",      ico:"✎", faz:editorDeMenu },
    "sep",'''
n = '''    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    "sep",'''
assert s.count(v) == 1
s = s.replace(v, n)

# databaseCorrente prefere o banco que a tela esta mostrando
v = '''function databaseCorrente() {
  return (est.atual && est.atual.db) || est.database || (est.bancos || [])[0] || "";
}'''
n = '''/** O database sobre o qual a gestao trabalha.
 *
 * A ordem importa: o que a TELA esta mostrando ganha do que a arvore tem
 * selecionado, senao abrir a gestao de um banco e clicar em "Gerir" levaria
 * para outro. O primeiro da lista e ultimo recurso, para quem acabou de
 * entrar e nao escolheu nada. */
function databaseCorrente() {
  return est.database || (est.atual && est.atual.db) || (est.bancos || [])[0] || "";
}'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
