# Extrair os pedidos do PENDENCIAS.md
# 29/08 03:15

import io,json,re,html
linhas=io.open('docs/PENDENCIAS.md',encoding='utf-8').read().split('\n')
itens=[]
for l in linhas:
    m=re.match(r'^\|\s*(☑️|◐|☐)\s*\|\s*(\d+)\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*$', l)
    if m:
        itens.append({'e':m.group(1),'n':int(m.group(2)),'p':m.group(3),'s':m.group(4)})
print(len(itens))
from collections import Counter
print(Counter(i['e'] for i in itens))
# numeros faltando
ns=sorted(i['n'] for i in itens)
print('duplicados:', [n for n in set(ns) if ns.count(n)>1])
print('faltando:', [n for n in range(1,max(ns)+1) if n not in ns])
json.dump(itens, io.open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/pedidos.json','w',encoding='utf-8'), ensure_ascii=False)
