# Trace where an extra brace opens
# 29/08 17:23

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text().splitlines()
nivel = 0
for i, l in enumerate(linhas, 1):
    c = re.sub(r'//.*$', '', l); c = re.sub(r'"(\\.|[^"\\])*"', '""', c)
    antes = nivel
    nivel += c.count("{") - c.count("}")
    if 1 <= antes <= 1 and nivel == 2 and re.match(r"^    (pub )?fn ", l):
        pass
    if i > 1700 and antes == 1 and nivel >= 2 and not re.match(r"^    (pub )?fn |^    ///|^\s*$", l):
        print(f"{i}: nivel {antes}->{nivel} :: {l[:90]}")
        if i > 2035: break
