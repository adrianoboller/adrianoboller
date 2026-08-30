# Update UI, restart, re-probe
# 28/08 14:29

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''function discosHtml(m) {
  const apertados = new Set(m.apertados || []);
  return barrasCheias((m.discos || []).map(d => ({
    nome: d.caminho + (d.montagem && d.montagem !== d.caminho ? `  ·  ${d.montagem}` : ""),
    percentual: Number(d.usado_percentual),
    alerta: apertados.has(d.caminho),
    texto: `${fmtBytes(d.livre_kb * 1024)} livres de ${fmtBytes(d.total_kb * 1024)}`,
  })));
}'''
b='''function discosHtml(m) {
  const apertados = new Set(m.apertados || []);
  return barrasCheias((m.discos || []).map(d => ({
    nome: d.caminho + (d.montagem && d.montagem !== d.caminho ? `  ·  ${d.montagem}` : ""),
    percentual: Number(d.usado_percentual),
    alerta: apertados.has(d.caminho),
    // O denominador é o alcançável (usado + livre), e não o tamanho do disco:
    // reserva de sistema de arquivos e cota não estão à disposição de ninguém,
    // e contá-las como livres é o que faz um disco cheio parecer vazio.
    texto: `${fmtBytes(d.livre_kb * 1024)} livres de ${fmtBytes((d.utilizavel_kb ?? d.total_kb) * 1024)}`
      + (d.reservado_kb ? `  ·  ${fmtBytes(d.reservado_kb * 1024)} reservados` : ""),
  })));
}'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
