# Renumber the FFI request and regenerate
# 30/08 16:44

p='docs/PENDENCIAS.md'
ls=open(p,encoding='utf-8').read().split('\n')
# O zelador entrou primeiro (commit a372b8d) e fica com o 149. A frente do
# embutido branchou antes dele existir e nao tinha como saber: vira 151, que e
# o proximo livre depois do 150 (a causa-raiz do zelador).
for i,l in enumerate(ls):
    if l.startswith('| ☑️ | 149 |') and 'embutido' in l:
        ls[i]=l.replace('| 149 |','| 151 |',1)
        print("embutido: 149 -> 151")
        break
open(p,'w',encoding='utf-8').write('\n'.join(ls))
