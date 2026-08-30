# Renumber the colliding requests and regenerate
# 30/08 17:11

p='phxsql/docs/PENDENCIAS.md'
ls=open(p,encoding='utf-8').read().split('\n')
# O zelador (149) e a causa-raiz (150) entraram primeiro. A frente do SQLite
# branchou antes deles e avisou que ia colidir: os dela viram 152 e 153.
mudou=[]
for i,l in enumerate(ls):
    if l.startswith('| ☑️ | 149 |') and 'SQLite' in l:
        ls[i]=l.replace('| 149 |','| 152 |',1); mudou.append('SQLite: 149 -> 152')
    elif l.startswith('| ☑️ | 150 |') and 'VM' in l:
        ls[i]=l.replace('| 150 |','| 153 |',1); mudou.append('VM/Wine: 150 -> 153')
open(p,'w',encoding='utf-8').write('\n'.join(ls))
print('\n'.join(mudou) if mudou else "NADA MUDOU -- conferir")
