# Measure which operations have a screen
# 28/08 15:14

import re
ui=open('crates/phxsql-server/ui/index.html').read()
srv=open('crates/phxsql-server/src/servidor.rs').read()
i=srv.index('fn executar(&self, op: &str'); j=srv.index('// ------------------------------------------------------------ ajudantes', i)
ops=[]
for m in re.finditer(r'^\s{12}((?:"[a-zA-Z_0-9]+"\s*\|\s*)*"[a-zA-Z_0-9]+")\s*=>', srv[i:j], re.M):
    ops.append(re.findall(r'"([^"]+)"', m.group(1)))
sem=[o for o in ops if not any(re.search(r'["\'`]%s["\'`]' % re.escape(n), ui) for n in o)]
print("despachadas:", len(ops), "| sem tela:", len(sem))
for o in sem: print("  ", "|".join(o))
