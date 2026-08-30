# Clear monitor in folha; syntax-check the page JS
# 28/08 14:27

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''  $("#painel").onclick = null;
  $("#titulo").textContent = titulo;'''
b='''  $("#painel").onclick = null;
  // E o mesmo caso do onclick: o relogio do monitor continuaria batendo numa
  // tela que nao tem mais onde desenhar.
  pararMonitor();
  $("#titulo").textContent = titulo;'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
