# Resolve the dossier, changelog and guard catalog
# 30/08 17:40

import subprocess, re
raiz='/home/user/adrianoboller/'; CAT='phxsql/bancada/guardas/catalogo.py'
def ler(rev): return subprocess.run(['git','-C',raiz,'show',f'{rev}:{CAT}'],capture_output=True,text=True,check=True).stdout
meu, dele = ler('HEAD'), ler('worktree-agent-a6b26a5742af8179c')
def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"',t))
novos=sorted(ids(dele)-ids(meu), key=lambda g: dele.index(f'"id": "{g}"'))
print("faltam em HEAD:", novos)
def bloco(t,g):
    m=re.search(r'"id":\s*"%s"'%re.escape(g),t)
    ini=t.rindex('\n    {\n',0,m.start())+1
    cab=re.search(r'(?:^    # [^\n]*\n)+\Z', t[:ini], re.M)
    if cab: ini=cab.start()
    return t[ini:t.index('\n    },\n',m.start())+len('\n    },\n')]
blocos=[bloco(dele,g) for g in novos]
for b,g in zip(blocos,novos): assert b.count('"id":')==1, g
corpo=meu.rstrip()[:-1].rstrip('\n')
novo=corpo+'\n'+''.join(blocos)+']\n'
n=[0]
novo=re.sub(r'^    # \d+\. (.+)$',lambda m:(n.__setitem__(0,n[0]+1),f'    # {n[0]}. {m.group(1)}')[1],novo,flags=re.M)
open(raiz+CAT,'w',encoding='utf-8').write(novo)
