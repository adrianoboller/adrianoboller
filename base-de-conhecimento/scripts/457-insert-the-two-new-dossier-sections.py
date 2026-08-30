# Insert the two new dossier sections
# 28/08 15:13

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/sec17.html').read()
marca='<!-- ============================= 19 ============================= -->'
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)
# sumario
a='''    <li><a href="#s19"><span class="n">19</span> Bancada</a></li>'''
b='''    <li><a href="#s17"><span class="n">17</span> A máquina embaixo</a></li>
    <li><a href="#s18"><span class="n">18</span> DbLink</a></li>
    <li><a href="#s19"><span class="n">19</span> Bancada</a></li>'''
assert a in s, [l for l in s.split('\n') if 'href="#s19"' in l]
s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
