# Locate unbalanced region after login merge
# 29/08 17:49

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text().splitlines()
nivel = 0; ult = None
for i, l in enumerate(linhas, 1):
    c = re.sub(r'//.*$', '', l); c = re.sub(r'"(\\.|[^"\\])*"', '""', c)
    if re.match(r"^    (pub )?fn ", l):
        if nivel != 1:
            print(f"divergencia na linha {i}; a fn anterior comecou em {ult}"); break
        ult = i
    nivel += c.count("{") - c.count("}")
