# Fix selector and restart
# 27/08 20:52

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
s=s.replace('''  // O campo "Database" do login abre a arvore ja nesse banco.
  if (est.database) {
    const no = [...document.querySelectorAll(".no.banco")]
      .find(n => n.textContent.trim() === est.database);
    if (no) no.click();
  }''','''  // O campo "Database" do login abre a arvore ja na primeira tabela
  // desse banco, em vez de deixar o usuario procurar.
  if (est.database) {
    const primeira = document.querySelector(
      `.no.tab[data-db="${CSS.escape(est.database)}"]`);
    if (primeira) primeira.click();
  }''')
open(p,'w').write(s)
