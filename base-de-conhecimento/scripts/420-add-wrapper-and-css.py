# Add wrapper and CSS
# 28/08 14:26

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

# 1) embrulho para poder repintar so os monitores
a='''  return `<div class="kpis">${kpis}</div><div class="cartas" id="cartas">
    ${maquinaHtml(maquina)}'''
b='''  return `<div class="kpis">${kpis}</div><div class="cartas" id="cartas">
    <div id="maquina" class="passa">${maquinaHtml(maquina)}</div>'''
assert a in s; s=s.replace(a,b,1)

# 2) CSS
a='''.vazioc{color:var(--texto-3);font-size:12.5px;font-style:italic;padding:14px 0}'''
b='''.vazioc{color:var(--texto-3);font-size:12.5px;font-style:italic;padding:14px 0}
/* display:contents faz o embrulho sumir do grid: os cartoes da maquina viram
   filhos diretos de .cartas, e sao repintados sozinhos a cada leitura sem que
   o embrulho vire uma celula gorda no meio do painel. */
.passa{display:contents}
.medidores{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));
           gap:10px;margin-bottom:16px}
.medidores svg{width:100%;max-width:172px;margin:0 auto;height:auto;display:block}'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
