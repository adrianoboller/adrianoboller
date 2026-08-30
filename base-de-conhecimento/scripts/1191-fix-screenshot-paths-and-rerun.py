# Fix screenshot paths and rerun
# 29/08 18:46

import pathlib
p = pathlib.Path("$S/integra/tlm.mjs"); t = p.read_text()
t = t.replace("\${'\$S'}/integra/", "$S/integra/")
p.write_text(t)
print("caminhos:", [l for l in t.splitlines() if "screenshot" in l])
