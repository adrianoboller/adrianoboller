# Resolve additive conflicts, summarize servidor.rs
# 29/08 18:39

import re, pathlib
# Aditivos: variante de erro nova e familia de permissao nova.
for a in ["crates/phxsql-core/src/error.rs","crates/phxsql-server/src/usuarios.rs"]:
    p = pathlib.Path(a); t = p.read_text()
    t, n = re.subn(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", lambda m: m.group(1)+m.group(2), t, flags=re.S)
    p.write_text(t); print(f"{a}: {n} aditivo(s)")
