# Drop provisional op, keep definitive
# 29/08 17:33

import re, pathlib
# Fica a op definitiva do motor (`replicacao_testar`); a provisoria do wizard
# sai, como o proprio autor dela pediu no comentario.
for a in ["crates/phxsql-server/src/catalogo.rs","crates/phxsql-server/src/servidor.rs"]:
    p = pathlib.Path(a); t = p.read_text()
    t2, n = re.subn(r"<<<<<<< [^\n]*\n(.*?)=======\n.*?>>>>>>> [^\n]*\n", lambda m: m.group(1), t, flags=re.S)
    p.write_text(t2); print(f"{a}: {n} conflito(s) resolvidos ficando com a definitiva")
