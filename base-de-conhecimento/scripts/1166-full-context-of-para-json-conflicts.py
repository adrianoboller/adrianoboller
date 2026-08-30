# Full context of para_json conflicts
# 29/08 18:06

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/config.rs").read_text()
ms = list(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S))
for i in (3, 4):
    m = ms[i]
    ini = max(0, m.start()-400)
    print(f"########## CONFLITO {i+1} — CONTEXTO ANTES ##########")
    print(t[ini:m.start()].rstrip()[-400:])
    print(f"---------- HEAD ----------"); print(m.group(1).rstrip())
    print(f"---------- RAMO ----------"); print(m.group(2).rstrip())
    print(f"---------- DEPOIS ----------"); print(t[m.end():m.end()+300].rstrip()[:300]); print()
