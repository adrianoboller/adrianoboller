# Resolve conflicts and detect lost fields
# 29/08 18:07

import re, pathlib
p = pathlib.Path("crates/phxsql-server/src/config.rs"); t = p.read_text()
n = 0
def resolve(m):
    global n; n += 1
    # 4 = o para_json reescrito pelo ramo (corrige as quatro telas que recebiam
    # dado mentiroso); os demais sao aditivos.
    return m.group(2) if n == 4 else m.group(1) + m.group(2)
t = re.sub(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", resolve, t, flags=re.S)
p.write_text(t); print("config.rs resolvido")

# usuarios.rs: aditivo
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs"); t = p.read_text()
t = re.sub(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", lambda m: m.group(1)+m.group(2), t, flags=re.S)
p.write_text(t); print("usuarios.rs resolvido")
