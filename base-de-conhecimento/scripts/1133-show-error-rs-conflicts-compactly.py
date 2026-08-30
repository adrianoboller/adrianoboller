# Show error.rs conflicts compactly
# 29/08 17:18

import re, pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
t = p.read_text()
for i, m in enumerate(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S), 1):
    print(f"--- conflito {i} ---")
    print("HEAD:", m.group(1)[:260].rstrip())
    print("RAMO:", m.group(2)[:260].rstrip())
