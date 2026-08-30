# Summarize servidor.rs conflicts
# 29/08 17:20

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
    h, r = m.group(1).rstrip(), m.group(2).rstrip()
    print(f"===== {i} =====")
    print("HEAD:", (h[:200] + ("…" if len(h)>200 else "")).replace("\n"," | "))
    print("RAMO:", (r[:200] + ("…" if len(r)>200 else "")).replace("\n"," | "))
