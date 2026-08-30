# Auto-resolve additive conflicts in servidor.rs
# 29/08 17:21

import re, pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
SEMANTICOS = {4, 6, 7}   # laco da replica, portao do papel, papel no estado
n = 0
def resolve(m):
    global n; n += 1
    if n in SEMANTICOS:
        return m.group(0)           # deixa para resolver a mao
    return m.group(1) + m.group(2)  # aditivo: ficam os dois lados
t = re.sub(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", resolve, t, flags=re.S)
p.write_text(t)
print("conflitos restantes:", t.count("<<<<<<<"))
