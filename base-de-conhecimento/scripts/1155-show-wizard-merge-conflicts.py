# Show wizard merge conflicts
# 29/08 17:33

import re, pathlib
for a in ["crates/phxsql-server/src/catalogo.rs","crates/phxsql-server/src/servidor.rs"]:
    t = pathlib.Path(a).read_text()
    print(f"##### {a}")
    for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
        print(f"-- {i} HEAD --\n{m.group(1).rstrip()[:330]}")
        print(f"-- {i} RAMO --\n{m.group(2).rstrip()[:330]}\n")
