# Apply the UI fixes for real
# 28/08 14:32

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''  const p = Math.max(0, Math.min(100, Number(percentual) || 0));
  const R = 46, cx = 60, cy = 58, esp = 11;'''
b='''  const p = Math.max(0, Math.min(100, Number(percentual) || 0));
  // O arco abre 240°, e as duas pontas caem em baixo, a y = cy + R·sen(135°).
  // Com cy=58 e R=46 elas caíam em 90, que era exatamente a linha do detalhe —
  // o traço laranja passava por cima do texto. Subir o centro e descer o
  // detalhe separa os dois.
  const R = 42, cx = 60, cy = 50, esp = 10;'''
assert a in s; s=s.replace(a,b,1)
a='''  return `<svg viewBox="0 0 120 ${detalhe ? 96 : 84}" role="img"'''
b='''  return `<svg viewBox="0 0 120 ${detalhe ? 104 : 86}" role="img"'''
assert a in s; s=s.replace(a,b,1)
a='''    ${detalhe ? `<text x="${cx}" y="${cy + 33}" text-anchor="middle" font-size="10"
          fill="currentColor" opacity=".6">${esc(detalhe)}</text>` : ""}'''
b='''    ${detalhe ? `<text x="${cx}" y="${cy + 49}" text-anchor="middle" font-size="9.5"
          fill="currentColor" opacity=".6">${esc(detalhe)}</text>` : ""}'''
assert a in s; s=s.replace(a,b,1)
a='''function barrasCheias(itens) {
  if (!itens.length) return `<div class="vazioc">sem dados ainda</div>`;
  const L = 360, alt = 40;'''
b='''function barrasCheias(itens, largura = 1180) {
  if (!itens.length) return `<div class="vazioc">sem dados ainda</div>`;
  // O SVG escala o desenho INTEIRO, texto junto: um viewBox de 360 esticado
  // até a largura da carta larga faz 11 px virarem 38 px. A largura tem de
  // nascer perto da real — é a mesma armadilha anotada em `barras`.
  const L = largura, alt = 40;'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
