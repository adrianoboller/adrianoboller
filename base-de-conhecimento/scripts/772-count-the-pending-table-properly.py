# Count the pending table properly
# 28/08 20:02

import re, pathlib
s = pathlib.Path("docs/PENDENCIAS.md").read_text()
linhas = [l for l in s.splitlines() if re.match(r"^\| (☑️|◐|☐) \| \d+ \|", l)]
from collections import Counter
c = Counter(l.split("|")[1].strip() for l in linhas)
ids = [int(l.split("|")[2].strip()) for l in linhas]
print(c, "total", len(linhas), "ids unicos", len(set(ids)))
dup = [i for i in set(ids) if ids.count(i) > 1]
print("duplicados:", dup)
