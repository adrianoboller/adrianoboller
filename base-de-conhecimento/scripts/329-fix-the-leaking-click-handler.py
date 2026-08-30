# Fix the leaking click handler
# 28/08 11:40

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

# 1. o tratador vai para o container das operacoes, nao para o painel
v = '''  $("#painel").onclick = ev => {
    const b = ev.target.closest(".op");
    if (!b) return;
    const it = itens[+b.dataset.op];
    if (!it.faz) return aindaNao(it);
    Promise.resolve().then(it.faz).catch(e => avisar(String(e), true));
  };'''
n = '''  // No container das operacoes, e nao no `#painel`: o `folha()` troca o
  // conteudo do painel mas nao o ELEMENTO, entao um `onclick` posto ali
  // sobrevive a troca de tela e dispara na tela seguinte.
  $$("#painel .ops").forEach(caixa => caixa.onclick = ev => {
    const b = ev.target.closest(".op");
    if (!b) return;
    const it = itens[+b.dataset.op];
    if (!it.faz) return aindaNao(it);
    Promise.resolve().then(it.faz).catch(e => avisar(String(e), true));
  });'''
assert s.count(v) == 1
s = s.replace(v, n)

# 2. e o folha() limpa o que a tela anterior tiver pendurado, por garantia
v = '''function folha(titulo, subtitulo, corpoHtml) {
  $("#abas").innerHTML = "";'''
n = '''function folha(titulo, subtitulo, corpoHtml) {
  $("#abas").innerHTML = "";
  // Trocar o `innerHTML` troca o conteudo, nao o elemento: um `onclick` que a
  // tela anterior tenha pendurado no proprio `#painel` continuaria ali e
  // dispararia nesta. Ja aconteceu -- a gestao do banco levava o clique das
  // telas seguintes.
  $("#painel").onclick = null;'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
