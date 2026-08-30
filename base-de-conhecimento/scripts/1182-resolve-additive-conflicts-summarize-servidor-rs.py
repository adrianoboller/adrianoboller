# Resolve additive conflicts, summarize servidor.rs
# 29/08 18:39

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
    h, r = m.group(1).rstrip(), m.group(2).rstrip()
    print(f"--{i}-- HEAD: {h[:150].replace(chr(10),' | ')}")
    print(f"      RAMO: {r[:150].replace(chr(10),' | ')}")
