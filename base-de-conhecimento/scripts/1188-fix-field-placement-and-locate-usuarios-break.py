# Fix field placement and locate usuarios break
# 29/08 18:40

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/usuarios.rs").read_text().splitlines()
n = 0
for i, l in enumerate(linhas, 1):
    c = re.sub(r'//.*$', '', l); c = re.sub(r'"(\\.|[^"\\])*"', '""', c)
    n += c.count("{") - c.count("}")
    if i > 90 and n < 2:
        print(f"nivel caiu para {n} na linha {i}: {l[:80]}"); break
