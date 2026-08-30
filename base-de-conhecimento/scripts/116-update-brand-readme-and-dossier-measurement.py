# Update brand readme and dossier measurement
# 27/08 20:11

import os
tam = os.path.getsize('crates/phxsql-server/ui/index.html')
kb = round(tam/1024)
print('interface:', tam, 'B =', kb, 'KB')
p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
velho = 'mais 34 KB de interface'
assert s.count(velho)==1
open(p,'w').write(s.replace(velho, f'mais {kb} KB de interface'))
