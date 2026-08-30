# Inspect config and usuarios conflicts
# 29/08 18:06

import re, pathlib
for a in ["crates/phxsql-server/src/usuarios.rs","crates/phxsql-server/src/config.rs"]:
    t = pathlib.Path(a).read_text()
    print(f"##### {a}")
    for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
        print(f"-- {i} HEAD --\n{m.group(1).rstrip()[:260]}")
        print(f"-- {i} RAMO --\n{m.group(2).rstrip()[:260]}\n")
