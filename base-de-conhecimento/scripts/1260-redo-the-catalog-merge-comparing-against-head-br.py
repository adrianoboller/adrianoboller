# Redo the catalog merge comparing against HEAD, bringing constants along
# 30/08 06:28

import subprocess, re
raiz='/home/user/adrianoboller/'; CAT='phxsql/bancada/guardas/catalogo.py'
def ler(rev): return subprocess.run(['git','-C',raiz,'show',f'{rev}:{CAT}'],capture_output=True,text=True,check=True).stdout
meu, dele = ler('HEAD'), ler('worktree-agent-a365475aaca79f7fe')
def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"',t))
# O que falta e o que HEAD nao tem -- comparar com a base do merge trouxe 18
# guardas que HEAD ja carregava, porque a base e anterior ao merge do fsync.
novos=sorted(ids(dele)-ids(meu), key=lambda g: dele.index(f'"id": "{g}"'))
print("faltam em HEAD:", novos)
def bloco(t,g):
    m=re.search(r'"id":\s*"%s"'%re.escape(g),t); ini=None
    for mm in re.finditer(r'^    # \d+\. .+$',t[:m.start()],re.M): ini=mm.start()
    return t[ini:t.index('\n    },\n',m.start())+len('\n    },\n')]
blocos=[bloco(dele,g) for g in novos]

# Constante de modulo que o bloco usa e HEAD nao tem: vem junto, senao o
# catalogo importa quebrado -- foi o que acabou de acontecer.
faltando=[]
for nome in set(re.findall(r'\b([A-Z][A-Z0-9_]{4,})\b', ''.join(blocos))):
    if nome not in meu and re.search(rf'^{nome}\s*=', dele, re.M):
        m=re.search(rf'^{nome}\s*=.*?(?=\n[A-Z_]|\n\n[A-Z#]|\nGUARDAS)', dele, re.M|re.S)
        faltando.append(m.group(0).rstrip()+'\n\n')
print("constantes trazidas junto:", [f.split('=')[0].strip() for f in faltando])

corpo=meu.rstrip()[:-1].rstrip('\n')
if faltando:
    marca=re.search(r'^(GUARDAS|[A-Z_]+)\s*(?::[^=]+)?=\s*\[', corpo, re.M)
    corpo=corpo[:marca.start()]+''.join(faltando)+corpo[marca.start():]
novo=corpo+'\n'+''.join(blocos)+']\n'
n=[0]
novo=re.sub(r'^    # \d+\. (.+)$',lambda m:(n.__setitem__(0,n[0]+1),f'    # {n[0]}. {m.group(1)}')[1],novo,flags=re.M)
open('bancada/guardas/catalogo.py','w',encoding='utf-8').write(novo)
print("gravado")
