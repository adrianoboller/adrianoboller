# Resolve catalog and additive docs
# 30/08 06:28

import subprocess, re
raiz='/home/user/adrianoboller/'
def ler(rev,cam):
    return subprocess.run(['git','-C',raiz,'show',f'{rev}:{cam}'],capture_output=True,text=True,check=True).stdout
CAT='phxsql/bancada/guardas/catalogo.py'
base=subprocess.run(['git','-C',raiz,'merge-base','HEAD','worktree-agent-a365475aaca79f7fe'],
                    capture_output=True,text=True,check=True).stdout.strip()
b,meu,dele=ler(base,CAT),ler('HEAD',CAT),ler('worktree-agent-a365475aaca79f7fe',CAT)
def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"',t))
novos=sorted(ids(dele)-ids(base),key=lambda g: dele.index(f'"id": "{g}"'))
def bloco(t,g):
    m=re.search(r'"id":\s*"%s"'%re.escape(g),t); ini=None
    for mm in re.finditer(r'^    # \d+\. .+$',t[:m.start()],re.M): ini=mm.start()
    return t[ini:t.index('\n    },\n',m.start())+len('\n    },\n')]
corpo=meu.rstrip()[:-1].rstrip('\n')
novo=corpo+'\n'+''.join(bloco(dele,g) for g in novos)+']\n'
n=[0]
novo=re.sub(r'^    # \d+\. (.+)$',lambda m:(n.__setitem__(0,n[0]+1),f'    # {n[0]}. {m.group(1)}')[1],novo,flags=re.M)
open('bancada/guardas/catalogo.py','w',encoding='utf-8').write(novo)
print(f"catalogo: +{len(novos)} guardas da frente da trava ({novos})")

# Documentos aditivos: os dois lados ficam.
for cam in ['CHANGELOG.md','docs/DESEMPENHO.md','docs/PENDENCIAS.md']:
    ls=open(cam,encoding='utf-8').read().split('\n'); out=[];i=0;c=0
    while i<len(ls):
        if ls[i].startswith('<<<<<<<'):
            i+=1;a=[]
            while not ls[i].startswith('======='): a.append(ls[i]);i+=1
            i+=1;z=[]
            while not ls[i].startswith('>>>>>>>'): z.append(ls[i]);i+=1
            i+=1;out.extend(a);out.extend(z);c+=1
        else: out.append(ls[i]);i+=1
    open(cam,'w',encoding='utf-8').write('\n'.join(out))
    print(f"{cam}: {c} conflitos, dois lados guardados")
