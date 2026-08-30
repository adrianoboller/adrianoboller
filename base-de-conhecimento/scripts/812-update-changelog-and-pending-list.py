# Update changelog and pending list
# 28/08 20:35

import re, pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
linhas = [l for l in s.splitlines() if re.match(r"^\| (☑️|◐|☐) \| \d+ \|", l)]
from collections import Counter
c = Counter(l.split("|")[1].strip() for l in linhas)
total = len(linhas)
novo = f"**{c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados**, de {total} pedidos."
s = re.sub(r"\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.", novo, s)
p.write_text(s)
print(novo)
