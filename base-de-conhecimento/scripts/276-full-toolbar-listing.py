# Full toolbar listing
# 28/08 11:04

import re, pathlib
s = pathlib.Path('crates/phxsql-server/ui/index.html').read_text()
bloco = s.split('const FERRAMENTAS = [')[1].split('\n];')[0]
itens = re.findall(r'\{ ico:"(\w+)",\s+rot:"([^"]+)",\s+cor:"[^"]+",\s*faz:([^\s,}]+)', bloco)
print('ferramentas:', len(itens))
for i, r, f in itens:
    print('  ', r.ljust(12), '·', 'apagada' if f == 'null' else 'liga ' + f)
