# Add the pending items and recount
# 28/08 21:26

import re, pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
linhas = [l for l in s.splitlines() if re.match(r"^\| (☑️|◐|☐) \| \d+ \|", l)]
from collections import Counter
c = Counter(l.split("|")[1].strip() for l in linhas)
novo = f"**{c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados**, de {len(linhas)} pedidos."
s = re.sub(r"\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.", novo, s)
p.write_text(s)
print(novo)
