# Fill the new fields at every call site
# 28/08 16:24

import re
p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
# Acrescenta os tres campos logo depois de `erro:` em cada construcao de Acesso.
# O sitio principal ganha os valores de verdade logo abaixo, num passo a parte.
padrao = re.compile(r'(self\.anotar\(&Acesso \{.*?\n(\s+)erro: [^\n]*\n)', re.S)
def troca(m):
    ident = m.group(2)
    return m.group(1) + f'{ident}database: String::new(),\n{ident}tabela: String::new(),\n{ident}codigo: 0,\n'
s2, n = padrao.subn(troca, s)
print("sitios ajustados:", n)
open(p,'w').write(s2)
