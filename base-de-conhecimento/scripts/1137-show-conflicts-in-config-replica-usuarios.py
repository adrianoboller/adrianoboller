# Show conflicts in config, replica, usuarios
# 29/08 17:20

import re, pathlib
for a in ["crates/phxsql-server/src/config.rs","crates/phxsql-server/src/replica.rs","crates/phxsql-server/src/usuarios.rs"]:
    t = pathlib.Path(a).read_text()
    print(f"########## {a}")
    for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
        print(f"--- HEAD ---\n{m.group(1).rstrip()[:500]}")
        print(f"--- RAMO ---\n{m.group(2).rstrip()[:500]}\n")
