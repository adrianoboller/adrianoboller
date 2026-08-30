# Count the editable labels exactly
# 28/08 13:12

import re, pathlib
s = pathlib.Path('crates/phxsql-server/ui/index.html').read_text()
menus = s.split('const MENUS = [')[1].split('\n];')[0]
# um rotulo por menu + um por item que nao e separador
n_menus = len(re.findall(r'^  \["', menus, re.M))
n_itens = len(re.findall(r'\{ rot:"', menus))
fer = s.split('const FERRAMENTAS = [')[1].split('\n];')[0]
n_fer = len(re.findall(r'\{ ico:"', fer))
print(f'{n_menus} menus + {n_itens} itens + {n_fer} ferramentas = {n_menus + n_itens + n_fer} rotulos')
