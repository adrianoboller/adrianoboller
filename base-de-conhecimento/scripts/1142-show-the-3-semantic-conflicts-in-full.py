# Show the 3 semantic conflicts in full
# 29/08 17:21

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
    print(f"########## CONFLITO {i} — HEAD ##########"); print(m.group(1).rstrip())
    print(f"########## CONFLITO {i} — RAMO ##########"); print(m.group(2).rstrip()); print()
