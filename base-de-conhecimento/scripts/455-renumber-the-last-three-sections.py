# Renumber the last three sections
# 28/08 15:10

import re
p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
# renumera 19->21, 18->20, 17->19 (de tras para frente, senao colidem)
for velho, novo in [(19,21),(18,20),(17,19)]:
    s=s.replace(f'<!-- ============================= {velho} ============================= -->',
                f'<!-- ============================= {novo} ============================= -->',1)
    s=s.replace(f'<section id="s{velho}">', f'<section id="s{novo}">',1)
    s=s.replace(f'<div class="rotulo"><span class="num">{velho}</span>',
                f'<div class="rotulo"><span class="num">{novo}</span>',1)
    s=s.replace(f'<li><a href="#s{velho}"><span class="n">{velho}</span>',
                f'<li><a href="#s{novo}"><span class="n">{novo}</span>',1)
open(p,'w').write(s)
