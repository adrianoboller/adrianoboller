# Add the joins section to the dossier
# 28/08 15:47

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
# renumera 21->22, 20->21, 19->20 para abrir espaco na 19
for velho, novo in [(21,22),(20,21),(19,20)]:
    s=s.replace(f'<!-- ============================= {velho} ============================= -->',
                f'<!-- ============================= {novo} ============================= -->',1)
    s=s.replace(f'<section id="s{velho}">', f'<section id="s{novo}">',1)
    s=s.replace(f'<div class="rotulo"><span class="num">{velho}</span>',
                f'<div class="rotulo"><span class="num">{novo}</span>',1)
    s=s.replace(f'<li><a href="#s{velho}"><span class="n">{velho}</span>',
                f'<li><a href="#s{novo}"><span class="n">{novo}</span>',1)
# as figuras 20 e 21 (bancada) viram 21 e 22
s=s.replace('<b>Figura 21.</b>','<b>Figura 22.</b>',1)
s=s.replace('<b>Figura 20.</b>','<b>Figura 21.</b>',1)
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/sec_juncao.html').read()
marca='<!-- ============================= 20 ============================= -->'
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)
a='''    <li><a href="#s20"><span class="n">20</span> Bancada</a></li>'''
b='''    <li><a href="#s19"><span class="n">19</span> Junções</a></li>
    <li><a href="#s20"><span class="n">20</span> Bancada</a></li>'''
assert a in s, [l for l in s.split('\n') if 'href="#s20"' in l]
s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
