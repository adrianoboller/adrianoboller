# Find all invalid code tags inside SVG text
# 28/08 18:09

import io,re
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
# procura <code> dentro de qualquer <text ...>...</text>
maus = [m.group(0)[:110] for m in re.finditer(r'<text\b[^>]*>.*?</text>', s, re.S) if '<code>' in m.group(0)]
print(f'{len(maus)} <text> com <code> dentro:')
for m in maus: print('  ', m.replace('\n',' '))
