# Verify toolbar and menu counts
# 28/08 11:04

import re, pathlib
s = pathlib.Path('crates/phxsql-server/ui/index.html').read_text()
bloco = s.split('const FERRAMENTAS = [')[1].split('\n];')[0]
itens = re.findall(r'\{ ico:"(\w+)",\s+rot:"([^"]+)",\s+cor:"([^"]+)",\s*faz:(\w+|\(\)[^,]*|null)', bloco)
print('ferramentas:', len(itens))
mortas = [r for _, r, _, f in itens if f == 'null']
print('apagadas:', len(mortas), mortas)
menus = re.findall(r'^  \["([^"]+)", "', s.split('const MENUS = [')[1].split('\n];')[0], re.M)
print('menus:', len(menus), menus)
