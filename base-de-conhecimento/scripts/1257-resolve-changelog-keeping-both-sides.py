# Resolve CHANGELOG keeping both sides
# 30/08 06:23

import re
# CHANGELOG e LEIA-ME: as duas frentes ACRESCENTARAM, ninguem substituiu.
# Descartar um lado apagaria o trabalho de quem nem tocou nas mesmas linhas --
# e o padrao certo aqui e o mesmo do merge por coluna: cada um fica com o seu.
for p in ['CHANGELOG.md']:
    linhas=open(p,encoding='utf-8').read().split('\n')
    saida=[]; i=0; n=0
    while i < len(linhas):
        if linhas[i].startswith('<<<<<<<'):
            i+=1; meu=[]
            while not linhas[i].startswith('======='): meu.append(linhas[i]); i+=1
            i+=1; dele=[]
            while not linhas[i].startswith('>>>>>>>'): dele.append(linhas[i]); i+=1
            i+=1; saida.extend(meu); saida.extend(dele); n+=1
        else:
            saida.append(linhas[i]); i+=1
    open(p,'w',encoding='utf-8').write('\n'.join(saida))
    print(f"{p}: {n} conflitos, os dois lados guardados")
