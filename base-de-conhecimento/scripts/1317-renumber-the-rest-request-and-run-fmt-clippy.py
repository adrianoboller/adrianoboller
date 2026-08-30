# Renumber the REST request and run fmt/clippy
# 30/08 17:19

p='phxsql/docs/PENDENCIAS.md'
ls=open(p,encoding='utf-8').read().split('\n')
for i,l in enumerate(ls):
    if l.startswith('| ☑️ | 149 |') and 'webservice' in l:
        ls[i]=l.replace('| 149 |','| 154 |',1); print("REST: 149 -> 154"); break
else: print("NAO ACHEI")
open(p,'w',encoding='utf-8').write('\n'.join(ls))
