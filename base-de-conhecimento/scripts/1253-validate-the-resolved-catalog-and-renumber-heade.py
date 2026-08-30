# Validate the resolved catalog and renumber headers
# 30/08 06:21

import re
p='bancada/guardas/catalogo.py'
s=open(p,encoding='utf-8').read()
n=[0]
s=re.sub(r'^    # (\d+)\. (.+)$', lambda m:(n.__setitem__(0,n[0]+1), f'    # {n[0]}. {m.group(2)}')[1], s, flags=re.M)
open(p,'w',encoding='utf-8').write(s)
print(f"{n[0]} cabecalhos renumerados em ordem")
