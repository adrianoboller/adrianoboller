# Recount toolbar
# 28/08 11:04

import re, pathlib
s = pathlib.Path('crates/phxsql-server/ui/index.html').read_text()
bloco = s.split('const FERRAMENTAS = [')[1].split('\n];')[0]
itens = re.findall(r'\{ ico:"\w+",\s*rot:"([^"]+)",\s*cor:"[^"]+",\s*faz:([^\s,}]+)', bloco)
vivas = [r for r, f in itens if f != 'null']
mortas = [r for r, f in itens if f == 'null']
print(f'{len(itens)} ferramentas · {len(vivas)} vivas · {len(mortas)} apagadas: {mortas}')
