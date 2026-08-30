# Resolve the catalog semantically by id, keeping both sides' new guards
# 30/08 06:21

import subprocess, re
def ler(rev):
    return subprocess.run(['git','show',f'{rev}:phxsql/bancada/guardas/catalogo.py'],
                          capture_output=True,text=True,check=True).stdout
base_rev=subprocess.run(['git','merge-base','HEAD','worktree-agent-ac3b21b44e7ecbdb9'],
                        capture_output=True,text=True,check=True).stdout.strip()
base, meu, dele = ler(base_rev), ler('HEAD'), ler('worktree-agent-ac3b21b44e7ecbdb9')

def ids(t): return set(re.findall(r'"id":\s*"([^"]+)"', t))
novos_dele = ids(dele) - ids(base)
print("base:", len(ids(base)), "| HEAD acrescentou:", sorted(ids(meu)-ids(base)),
      "| a frente acrescentou:", sorted(novos_dele))

# Recorta o bloco de cada guarda nova pelo comentario-cabecalho ate o fecho do dict.
def bloco(texto, gid):
    m = re.search(r'"id":\s*"%s"' % re.escape(gid), texto)
    ini = texto.rfind('    # ', 0, m.start())
    fim = texto.index('\n    },\n', m.start()) + len('\n    },\n')
    return texto[ini:fim]

blocos = [bloco(dele, g) for g in sorted(novos_dele, key=lambda g: dele.index(f'"id": "{g}"'))]
fecho = '\n]\n'
assert meu.rstrip().endswith(']'), meu[-60:]
corte = meu.rstrip()[:-1].rstrip('\n')
open('bancada/guardas/catalogo.py','w',encoding='utf-8').write(corte + '\n' + ''.join(blocos) + ']\n')
print(f"{len(blocos)} guardas da frente do fsync anexadas as de HEAD")
