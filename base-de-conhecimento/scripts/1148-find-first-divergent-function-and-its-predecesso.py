# Find first divergent function and its predecessor
# 29/08 17:23

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text().splitlines()
nivel = 0; anterior_fn = None
for i, l in enumerate(linhas, 1):
    c = re.sub(r'//.*$', '', l); c = re.sub(r'"(\\.|[^"\\])*"', '""', c)
    if re.match(r"^    (pub )?fn ", l):
        if nivel != 1:
            print(f"PRIMEIRA DIVERGENCIA linha {i} nivel {nivel}; fn anterior comecou em {anterior_fn}")
            break
        anterior_fn = i
    nivel += c.count("{") - c.count("}")
