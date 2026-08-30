# Wire the theme toggle
# 27/08 20:31

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
velho = '''// =====================================================================
$("#btEntrar").onclick = entrar;'''
novo = '''// =====================================================================
// Tema. Comeca no que o sistema pede e lembra a escolha por navegador --
// localStorage, que e do visitante e nunca chega ao servidor. Em janela
// anonima o acesso levanta excecao em vez de devolver vazio, entao toda
// leitura e escrita vai dentro de try.
// =====================================================================
const TEMAS = { escuro:"🌙", claro:"☀️" };

function lembrar(chave, valor) {
  try { localStorage.setItem(chave, valor); } catch {}
}
function lembrado(chave) {
  try { return localStorage.getItem(chave); } catch { return null; }
}

function aplicarTema(qual) {
  const claro = qual === "claro";
  document.documentElement.setAttribute("data-tema", claro ? "claro" : "escuro");
  // O icone mostra PARA ONDE o clique leva, nao onde se esta: no escuro
  // aparece o sol, porque clicar acende.
  $("#btTema").textContent = claro ? TEMAS.escuro : TEMAS.claro;
  $("#btTema").title = claro ? "Mudar para o tema escuro" : "Mudar para o tema claro";
  lembrar("phxsql-tema", claro ? "claro" : "escuro");
}

const sistemaClaro = () =>
  window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches;

aplicarTema(lembrado("phxsql-tema") || (sistemaClaro() ? "claro" : "escuro"));
$("#btTema").onclick = () =>
  aplicarTema(document.documentElement.getAttribute("data-tema") === "claro"
    ? "escuro" : "claro");

$("#btEntrar").onclick = entrar;'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
print('tema ok')
