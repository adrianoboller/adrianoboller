# Inspect error and usuarios conflicts
# 29/08 18:39

import re, pathlib
for a in ["crates/phxsql-core/src/error.rs","crates/phxsql-server/src/usuarios.rs"]:
    t = pathlib.Path(a).read_text()
    print(f"##### {a}")
    for m in re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S):
        print("HEAD:", m.group(1).rstrip()[:280]); print("RAMO:", m.group(2).rstrip()[:280]); print()
