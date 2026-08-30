# Fix the tree/panel race
# 28/08 10:59

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

v = '''async function montarArvore() {'''
n = '''/** Redesenha a arvore da esquerda.
 *
 * `abrirPainel` existe por causa de uma corrida real: a arvore terminava
 * SEMPRE clicando no Painel, e o `abrirAdmin` disparado por esse clique
 * chegava DEPOIS da tela que quem chamou ja tinha pintado -- criar uma tabela
 * voltava para a grade e a grade era substituida pelo painel meio segundo
 * depois. Quem vai pintar a propria tela passa `false`. */
async function montarArvore(abrirPainel = true) {'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''  // O painel e a primeira tela: quem entra ve o servidor inteiro antes de
  // escolher uma tabela.
  a.querySelector('[data-admin="painel"]').click();'''
n = '''  // O painel e a primeira tela: quem entra ve o servidor inteiro antes de
  // escolher uma tabela.
  if (abrirPainel) a.querySelector('[data-admin="painel"]').click();'''
assert s.count(v) == 1
s = s.replace(v, n)

# os dois pontos que pintam a propria tela em seguida
antes = s.count('''  await montarArvore();
  return gerirTabelas(db);''')
assert antes == 1
s = s.replace('''  await montarArvore();
  return gerirTabelas(db);''', '''  await montarArvore(false);
  return gerirTabelas(db);''')

antes = s.count('''      await montarArvore();
      return gerirTabelas(db);''')
assert antes == 1
s = s.replace('''      await montarArvore();
      return gerirTabelas(db);''', '''      await montarArvore(false);
      return gerirTabelas(db);''')
p.write_text(s)
print('ok')
