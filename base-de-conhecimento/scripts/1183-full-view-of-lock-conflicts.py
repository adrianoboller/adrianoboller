# Full view of lock conflicts
# 29/08 18:39

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
ms = list(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S))
for i in (3, 6):
    m = ms[i]
    print(f"########## {i+1} — ANTES ##########"); print(t[max(0,m.start()-260):m.start()].rstrip()[-260:])
    print(f"---- HEAD ----"); print(m.group(1).rstrip()[:700])
    print(f"---- RAMO ----"); print(m.group(2).rstrip()[:500])
    print(f"---- DEPOIS ----"); print(t[m.end():m.end()+200].rstrip()); print()
