# Locate brace imbalance
# 29/08 18:08

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text().splitlines()
nivel = 0; ult = None
for i, l in enumerate(linhas, 1):
    c = re.sub(r'//.*$', '', l); c = re.sub(r'"(\\.|[^"\\])*"', '""', c)
    if re.match(r"^    (pub )?fn ", l):
        if nivel != 1:
            print(f"divergencia linha {i} (nivel {nivel}); fn anterior em {ult}"); break
        ult = i
    nivel += c.count("{") - c.count("}")
