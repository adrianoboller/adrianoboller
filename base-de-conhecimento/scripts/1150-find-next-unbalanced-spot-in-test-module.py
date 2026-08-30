# Find next unbalanced spot in test module
# 29/08 17:23

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text().splitlines()
nivel = 0; ult = None
for i, l in enumerate(linhas, 1):
    c = re.sub(r'//.*$', '', l); c = re.sub(r'"(\\.|[^"\\])*"', '""', c)
    if i > 10912 and re.match(r"^    (pub )?(async )?fn ", l):
        if nivel != 1:
            print(f"divergencia linha {i} nivel {nivel}; fn anterior linha {ult}"); break
        ult = i
    nivel += c.count("{") - c.count("}")
