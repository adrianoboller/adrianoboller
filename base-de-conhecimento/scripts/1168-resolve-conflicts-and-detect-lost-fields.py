# Resolve conflicts and detect lost fields
# 29/08 18:07

import re, subprocess, pathlib
atual = set(re.findall(r'\("([a-z_]+)"', pathlib.Path("crates/phxsql-server/src/config.rs").read_text().split("pub fn para_json")[1].split("\n    }\n")[0]))
head = subprocess.run(["git","show","HEAD:phxsql/crates/phxsql-server/src/config.rs"],capture_output=True,text=True,cwd="/home/user/adrianoboller").stdout
antes = set(re.findall(r'\("([a-z_]+)"', head.split("pub fn para_json")[1].split("\n    }\n")[0]))
print("sumiram:", sorted(antes - atual))
