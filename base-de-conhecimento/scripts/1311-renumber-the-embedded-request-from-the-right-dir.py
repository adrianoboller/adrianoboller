# Renumber the embedded request from the right directory
# 30/08 16:44

p='docs/PENDENCIAS.md'
ls=open(p,encoding='utf-8').read().split('\n')
for i,l in enumerate(ls):
    if l.startswith('| ☑️ | 149 |') and 'embutido' in l:
        ls[i]=l.replace('| 149 |','| 151 |',1)
        print("embutido: 149 -> 151")
        break
else:
    print("NAO ACHEI a linha do embutido com 149")
open(p,'w',encoding='utf-8').write('\n'.join(ls))
