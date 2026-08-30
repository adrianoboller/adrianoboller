# Resolve additive conflicts and build
# 29/08 17:49

import re, pathlib
# Aditivos: as ops de idioma e as do firewall convivem.
for a in ["crates/phxsql-server/src/catalogo.rs","crates/phxsql-server/src/servidor.rs"]:
    p = pathlib.Path(a); t = p.read_text()
    t2, n = re.subn(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", lambda m: m.group(1)+m.group(2), t, flags=re.S)
    p.write_text(t2); print(f"{a}: {n} resolvido(s) mantendo os dois")
