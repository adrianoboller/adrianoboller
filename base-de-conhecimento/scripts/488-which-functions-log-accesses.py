# Which functions log accesses
# 28/08 16:25

import re
s=open('crates/phxsql-server/src/servidor.rs').read().split('\n')
fn=None
for i,l in enumerate(s,1):
    m=re.match(r'\s*(?:pub )?fn (\w+)', l)
    if m: fn=m.group(1)
    if 'self.anotar(&Acesso' in l: print(f"linha {i}: dentro de {fn}")
