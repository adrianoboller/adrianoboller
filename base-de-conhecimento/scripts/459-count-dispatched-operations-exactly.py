# Count dispatched operations exactly
# 28/08 15:14

import re
s=open('crates/phxsql-server/src/servidor.rs').read()
i=s.index('fn executar(&self, op: &str')
j=s.index('// ------------------------------------------------------------ ajudantes', i)
corpo=s[i:j]
# cada braco do match: "nome" | "alias" => ...
ops=[]
for m in re.finditer(r'^\s{12}((?:"[a-zA-Z_0-9]+"\s*\|\s*)*"[a-zA-Z_0-9]+")\s*=>', corpo, re.M):
    nomes=re.findall(r'"([^"]+)"', m.group(1))
    ops.append(nomes)
print("operacoes despachadas:", len(ops))
for o in ops: print("  ", " | ".join(o))
