# Redo the catalog resolution cleanly with correct headers
# 30/08 06:22

import subprocess, re
def ler(rev):
    return subprocess.run(['git','show',f'{rev}:phxsql/bancada/guardas/catalogo.py'],
                          capture_output=True,text=True,check=True).stdout
base_rev=subprocess.run(['git','merge-base','HEAD','worktree-agent-ac3b21b44e7ecbdb9'],
                        capture_output=True,text=True,check=True).stdout.strip()
base, meu, dele = ler(base_rev), ler('HEAD'), ler('worktree-agent-ac3b21b44e7ecbdb9')
def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"', t))
novos = sorted(ids(dele)-ids(base), key=lambda g: dele.index(f'"id": "{g}"'))

def bloco(texto, gid):
    m = re.search(r'"id":\s*"%s"' % re.escape(gid), texto)
    # O cabecalho e a linha `    # N. Titulo`, nao o traco que vem depois dela.
    ini = None
    for mm in re.finditer(r'^    # \d+\. .+$', texto[:m.start()], re.M):
        ini = mm.start()
    fim = texto.index('\n    },\n', m.start()) + len('\n    },\n')
    return texto[ini:fim]

corpo = meu.rstrip()
assert corpo.endswith(']')
corpo = corpo[:-1].rstrip('\n')
novo = corpo + '\n' + ''.join(bloco(dele,g) for g in novos) + ']\n'
n=[0]
novo=re.sub(r'^    # \d+\. (.+)$',
            lambda m:(n.__setitem__(0,n[0]+1), f'    # {n[0]}. {m.group(1)}')[1],
            novo, flags=re.M)
open('bancada/guardas/catalogo.py','w',encoding='utf-8').write(novo)
print(f"{len(novos)} guardas anexadas com cabecalho; {n[0]} numeradas em ordem")
