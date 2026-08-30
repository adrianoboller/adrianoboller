# Show the final gate conflict
# 29/08 18:08

import re, pathlib
t = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text()
m = re.search(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S)
print("HEAD:\n", m.group(1).rstrip()[:1400]); print("\nRAMO:\n", m.group(2).rstrip()[:700])
