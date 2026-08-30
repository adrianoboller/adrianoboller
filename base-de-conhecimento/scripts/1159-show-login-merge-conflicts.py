# Show login merge conflicts
# 29/08 17:49

import re, pathlib
for a in ["crates/phxsql-server/src/catalogo.rs","crates/phxsql-server/src/servidor.rs"]:
    t = pathlib.Path(a).read_text()
    n = len(re.findall(r"<<<<<<< ", t))
    print(f"##### {a}: {n}")
    for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
        print(f"-- {i} HEAD --\n{m.group(1).rstrip()[:200]}")
        print(f"-- {i} RAMO --\n{m.group(2).rstrip()[:200]}\n")
