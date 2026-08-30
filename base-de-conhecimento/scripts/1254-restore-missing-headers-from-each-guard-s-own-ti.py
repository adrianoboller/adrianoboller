# Restore missing headers from each guard's own title
# 30/08 06:22

import re
p='bancada/guardas/catalogo.py'
s=open(p,encoding='utf-8').read()
TRACO='    # ' + '-'*71

# Onde ficou so o traco (a resolucao levou o cabecalho junto), reconstroi o
# titulo a partir do proprio campo "titulo" da guarda -- que e o dado, e nao
# uma lembranca minha do que estava escrito.
def repoe(m):
    sep, corpo = m.group(1), m.group(2)
    tit = re.search(r'"titulo":\s*"([^"]+)"', corpo)
    return f'    # {tit.group(1)}\n{sep}{corpo}' if tit else m.group(0)

s = re.sub(r'(' + re.escape(TRACO) + r'\n)(\s*\{\n(?:.*?\n)*?    \},\n)',
           lambda m: m.group(0) if s[:m.start()].rstrip().endswith(('.', 'o', 'a', 's', 'e'))
           and re.search(r'#\s*\d+\.', s[max(0,m.start()-200):m.start()]) else repoe(m),
           s)
n=[0]
def renum(m):
    n[0]+=1
    return f'    # {n[0]}. {m.group(1)}'
s=re.sub(r'^    # (?:\d+\. )?(?!-)(.+)$\n(?=' + re.escape(TRACO) + r')', renum, s, flags=re.M)
open(p,'w',encoding='utf-8').write(s)
print(f"{n[0]} cabecalhos numerados")
