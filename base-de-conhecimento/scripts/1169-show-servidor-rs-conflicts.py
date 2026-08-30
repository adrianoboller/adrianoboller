# Show servidor.rs conflicts
# 29/08 18:07

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
    print(f"-- {i} HEAD --\n{m.group(1).rstrip()[:230]}")
    print(f"-- {i} RAMO --\n{m.group(2).rstrip()[:230]}\n")
