# Re-measure for the dossier
# 28/08 16:45

import re
srv=open('crates/phxsql-server/src/servidor.rs').read()
ui=open('crates/phxsql-server/ui/index.html').read()
i=srv.index('fn executar(&self, op: &str'); j=srv.index('// ------------------------------------------------------------ ajudantes', i)
ops=[]
for m in re.finditer(r'^\s{12}((?:"[a-zA-Z_0-9]+"\s*\|\s*)*"[a-zA-Z_0-9]+")\s*=>', srv[i:j], re.M):
    ops.append(re.findall(r'"([^"]+)"', m.group(1)))
sem=[o for o in ops if not any(re.search(r'["\'`]%s["\'`]' % re.escape(n), ui) for n in o)]
sem=[o for o in sem if o!=["buscar"]] + [["buscar"]]
print("operacoes:", len(ops), "| com tela:", len(ops)-len(sem), "| sem:", ["|".join(o) for o in sem])
