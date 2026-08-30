# Test dark theme, CSV and the request view
# 28/08 13:34

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
# Sequence e um contador: numero, nao texto.
antes = s.count('numero: /^Int|^UInt|^Real|^Decimal/.test(c.tipo),')
assert antes == 2, antes
s = s.replace('numero: /^Int|^UInt|^Real|^Decimal/.test(c.tipo),',
              'numero: /^Int|^UInt|^Real|^Decimal|^Sequence/.test(c.tipo),')
p.write_text(s)
print('Sequence conta como numero')
