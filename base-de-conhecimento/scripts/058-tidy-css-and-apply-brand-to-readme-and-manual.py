# Tidy CSS and apply brand to README and manual
# 27/08 19:17

import re
p="docs/dossie/dossie-phxsql.html"
s=open(p).read()
# normaliza a indentacao do bloco do toggle
def arruma(m):
    corpo="\n".join("  "+l.strip() for l in m.group(2).strip().split("\n"))
    return m.group(1)+"\n"+corpo+"\n}"
s=re.sub(r'(:root\[data-theme="dark"\]\{)\n(.*?)\n\}', arruma, s, count=1, flags=re.S)
open(p,'w').write(s)
