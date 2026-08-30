# Full view of field and method conflicts
# 29/08 18:07

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
ms = list(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S))
for i in (0, 2):
    print(f"########## {i+1} HEAD ##########"); print(ms[i].group(1).rstrip())
    print(f"########## {i+1} RAMO ##########"); print(ms[i].group(2).rstrip()); print()
