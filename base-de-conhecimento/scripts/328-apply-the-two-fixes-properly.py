# Apply the two fixes properly
# 28/08 11:36

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    { rot:"Editor de menu…",      ico:"✎", faz:editorDeMenu },
    "sep",'''
if s.count(v) == 1:
    s = s.replace(v, '''    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    "sep",''')
    print('tirei o duplicado de Ferramentas')

v = '''function databaseCorrente() {
  return (est.atual && est.atual.db) || est.database || (est.bancos || [])[0] || "";
}'''
if s.count(v) == 1:
    s = s.replace(v, '''/** O database sobre o qual a gestao trabalha.
 *
 * A ordem importa: o que a TELA esta mostrando ganha do que a arvore tem
 * selecionado, senao abrir a gestao de um banco e clicar em "Gerir" levaria
 * para outro. O primeiro da lista e ultimo recurso, para quem acabou de
 * entrar e nao escolheu nada. */
function databaseCorrente() {
  return est.database || (est.atual && est.atual.db) || (est.bancos || [])[0] || "";
}''')
    print('databaseCorrente reordenado')
p.write_text(s)
