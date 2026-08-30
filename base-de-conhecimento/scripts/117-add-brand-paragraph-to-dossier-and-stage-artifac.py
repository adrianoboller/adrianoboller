# Add brand paragraph to dossier and stage artifacts
# 27/08 20:11

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
velho = '''  <div class="nota">
    <p>Aberta sem servidor na origem, a página percebe pelo <code>GET /saude</code> e'''
novo = '''  <h3>A marca entra sem trazer dependência</h3>

  <p>A fênix está no cartão de entrada e na barra do topo, embutida no próprio
  <code>index.html</code> como <em>data URI</em> — a página continua sendo um arquivo
  só, e não há de onde buscar imagem. A CSP permite exatamente isso e nada mais:
  <code>img-src data:</code>.</p>

  <p>Os originais da marca não têm alfa: o fundo <code>#010418</code> vem pintado.
  Ele foi retirado desfazendo a pré-multiplicação — subtrai o fundo, tira
  <code>alfa = max(r,g,b)</code> e divide a cor por ele. Como o logotipo é brilho sobre
  quase preto, isso recupera a cor real de cada pixel e a borda do brilho sai suave,
  em vez de recortada com halo. É o que faz a fênix assentar sobre o painel
  <code>#0a1122</code> sem deixar emenda de retângulo. O fundo da interface passou a ser
  o <code>#010418</code> oficial: a marca manda sobre paleta inventada.</p>

  <div class="nota">
    <p>Aberta sem servidor na origem, a página percebe pelo <code>GET /saude</code> e'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
print('ok')
