# Insert machine UI into panel
# 28/08 14:26

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/ui_maquina.js').read()
marca='''async function vPainel() {
  const d = await api("painel");'''
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)

a='''async function vPainel() {
  const d = await api("painel");
  const s = d.resumo;
  est.painel = d;
'''
b='''async function vPainel() {
  // As duas chamadas em paralelo: o `painel` varre os bancos e o `sistema`
  // chama o `df`, e uma esperar a outra dobraria o tempo da tela por nada.
  const [d, maquina] = await Promise.all([api("painel"), lerMaquina()]);
  const s = d.resumo;
  est.painel = d;
  est.maquina = maquina;
'''
assert a in s; s=s.replace(a,b,1)

a='''  return `<div class="kpis">${kpis}</div><div class="cartas">
    ${carta("Operações por hora"'''
b='''  return `<div class="kpis">${kpis}</div><div class="cartas" id="cartas">
    ${maquinaHtml(maquina)}
    ${carta("Operações por hora"'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
