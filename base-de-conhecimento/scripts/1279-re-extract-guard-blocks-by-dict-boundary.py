# Re-extract guard blocks by dict boundary
# 30/08 06:42

import subprocess, re
raiz='/home/user/adrianoboller/'; CAT='phxsql/bancada/guardas/catalogo.py'
def ler(rev): return subprocess.run(['git','-C',raiz,'show',f'{rev}:{CAT}'],capture_output=True,text=True,check=True).stdout
meu, dele = ler('HEAD'), ler('worktree-agent-adcbe782ee242bc17')
def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"',t))
novos=sorted(ids(dele)-ids(meu), key=lambda g: dele.index(f'"id": "{g}"'))

def bloco(t,g):
    m=re.search(r'"id":\s*"%s"'%re.escape(g),t)
    # Recorta pelo DICIONARIO: a abertura `    {` imediatamente anterior ao id,
    # e o `    },` que o fecha. Ancorar no comentario varria a guarda vizinha
    # quando a de cima nao tinha cabecalho -- foi o que duplicou seis.
    ini=t.rindex('\n    {\n',0,m.start())+1
    cab=re.search(r'(?:^    # [^\n]*\n)+\Z', t[:ini], re.M)
    if cab: ini=cab.start()
    return t[ini:t.index('\n    },\n',m.start())+len('\n    },\n')]

blocos=[bloco(dele,g) for g in novos]
for b,g in zip(blocos,novos):
    assert b.count('"id":')==1, f"{g}: bloco varreu {b.count('\"id\":')} guardas"
corpo=meu.rstrip()[:-1].rstrip('\n')
novo=corpo+'\n'+''.join(blocos)+']\n'
n=[0]
novo=re.sub(r'^    # \d+\. (.+)$',lambda m:(n.__setitem__(0,n[0]+1),f'    # {n[0]}. {m.group(1)}')[1],novo,flags=re.M)
open('bancada/guardas/catalogo.py','w',encoding='utf-8').write(novo)
print(f"{len(blocos)} guardas anexadas, cada bloco com exatamente um id")
