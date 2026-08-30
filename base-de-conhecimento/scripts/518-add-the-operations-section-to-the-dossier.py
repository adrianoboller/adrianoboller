# Add the operations section to the dossier
# 28/08 16:44

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
for velho, novo in [(22,23),(21,22),(20,21)]:
    s=s.replace(f'<!-- ============================= {velho} ============================= -->',
                f'<!-- ============================= {novo} ============================= -->',1)
    s=s.replace(f'<section id="s{velho}">', f'<section id="s{novo}">',1)
    s=s.replace(f'<div class="rotulo"><span class="num">{velho}</span>',
                f'<div class="rotulo"><span class="num">{novo}</span>',1)
    s=s.replace(f'<li><a href="#s{velho}"><span class="n">{velho}</span>',
                f'<li><a href="#s{novo}"><span class="n">{novo}</span>',1)
# as figuras da bancada (21 e 22) empurram para 22 e 23
s=s.replace('<b>Figura 22.</b>','<b>Figura 23.</b>',1)
s=s.replace('<b>Figura 21.</b>','<b>Figura 22.</b>',1)
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/sec_oper.html').read()
marca='<!-- ============================= 21 ============================= -->'
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)
a='''    <li><a href="#s21"><span class="n">21</span> Bancada</a></li>'''
b='''    <li><a href="#s20"><span class="n">20</span> Operação</a></li>
    <li><a href="#s21"><span class="n">21</span> Bancada</a></li>'''
assert a in s, [l for l in s.split('\n') if 'href="#s21"' in l]
s=s.replace(a,b,1)
s=s.replace('  o DbLink em <code>docs/DBLINK.md</code>, as junções em <code>docs/JUNCOES.md</code>,',
            '  o DbLink em <code>docs/DBLINK.md</code>, as junções em <code>docs/JUNCOES.md</code>,\n  a revisão contra os motores maduros em <code>docs/COMPARACAO.md</code>,',1)
open(p,'w').write(s)
print('ok')
