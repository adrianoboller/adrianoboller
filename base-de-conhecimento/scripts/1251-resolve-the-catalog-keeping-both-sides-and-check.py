# Resolve the catalog keeping both sides and check for duplicate ids
# 30/08 06:20

import re
p='bancada/guardas/catalogo.py'
linhas=open(p,encoding='utf-8').read().split('\n')
saida=[]; i=0; resolvidos=0
while i < len(linhas):
    if linhas[i].startswith('<<<<<<<'):
        i+=1; meu=[]
        while not linhas[i].startswith('======='):
            meu.append(linhas[i]); i+=1
        i+=1; dele=[]
        while not linhas[i].startswith('>>>>>>>'):
            dele.append(linhas[i]); i+=1
        i+=1
        # Guarda de frente diferente e guarda acrescentada, nao substituida:
        # as duas ficam. Descartar uma perderia a prova que ela carrega.
        saida.extend(meu); saida.extend(dele); resolvidos+=1
    else:
        saida.append(linhas[i]); i+=1
s='\n'.join(saida)
# Os numeros dos comentarios sao rotulo, nao identidade -- renumera em ordem.
n=[0]
def renumera(m):
    n[0]+=1
    return f'    # {n[0]}. {m.group(2)}'
s=re.sub(r'^    # (\d+)\. (.+)$', renumera, s, flags=re.M)
open(p,'w',encoding='utf-8').write(s)
print(f"{resolvidos} conflitos resolvidos guardando os dois lados; {n[0]} guardas renumeradas")
