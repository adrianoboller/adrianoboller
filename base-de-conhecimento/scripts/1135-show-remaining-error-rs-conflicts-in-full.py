# Show remaining error.rs conflicts in full
# 29/08 17:19

import re, pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
t = p.read_text()
for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
    print(f"--- {i} HEAD ---"); print(m.group(1).rstrip()[:400])
    print(f"--- {i} RAMO ---"); print(m.group(2).rstrip()[:400]); print()
