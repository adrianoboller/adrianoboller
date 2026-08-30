# Resolve the guard catalog by id and take the dossier
# 30/08 16:53

import subprocess, re
raiz='/home/user/adrianoboller/'; CAT='phxsql/bancada/guardas/catalogo.py'
def ler(rev): return subprocess.run(['git','-C',raiz,'show',f'{rev}:{CAT}'],capture_output=True,text=True,check=True).stdout
meu, dele = ler('HEAD'), ler('worktree-agent-a474d0a48526428f6')
def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"',t))
novos=sorted(ids(dele)-ids(meu), key=lambda g: dele.index(f'"id": "{g}"'))
print("faltam em HEAD:", novos)
ID='"id":'
def bloco(t,g):
    m=re.search(r'"id":\s*"%s"'%re.escape(g),t)
    ini=t.rindex('\n    {\n',0,m.start())+1
    cab=re.search(r'(?:^    # [^\n]*\n)+\Z', t[:ini], re.M)
    if cab: ini=cab.start()
    return t[ini:t.index('\n    },\n',m.start())+len('\n    },\n')]
blocos=[bloco(dele,g) for g in novos]
for b,g in zip(blocos,novos):
    q=b.count(ID); assert q==1, "{}: bloco varreu {}".format(g,q)
corpo=meu.rstrip()[:-1].rstrip('\n')
novo=corpo+'\n'+''.join(blocos)+']\n'
n=[0]
novo=re.sub(r'^    # \d+\. (.+)$',lambda m:(n.__setitem__(0,n[0]+1),f'    # {n[0]}. {m.group(1)}')[1],novo,flags=re.M)
open('/home/user/adrianoboller/phxsql/bancada/guardas/catalogo.py','w',encoding='utf-8').write(novo)
print(f"{len(blocos)} guardas anexadas")
