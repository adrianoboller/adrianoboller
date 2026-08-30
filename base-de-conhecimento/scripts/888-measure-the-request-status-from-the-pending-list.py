# Measure the request status from the pending list
# 28/08 23:37

import re
s = open("docs/PENDENCIAS.md").read()
linhas = [l for l in s.splitlines() if re.match(r"^\| (☑️|◐|☐) \| \d+ \|", l)]
from collections import Counter
c = Counter(l.split("|")[1].strip() for l in linhas)
print(f"TOTAL: {len(linhas)} pedidos — {c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados")
print()
print("=== OS 15 MAIS RECENTES (113 a 127) ===")
for l in linhas:
    n = int(l.split("|")[2].strip())
    if n >= 108:
        est = l.split("|")[1].strip()
        titulo = l.split("|")[3].strip()
        titulo = re.sub(r"\*\*(.+?)\*\*", r"\1", titulo)[:95]
        print(f"  {est}  {n:>3}  {titulo}")
print()
print("=== O QUE FALTA (todos os ☐ e ◐) ===")
for l in linhas:
    est = l.split("|")[1].strip()
    if est in ("☐", "◐"):
        n = l.split("|")[2].strip()
        titulo = re.sub(r"\*\*(.+?)\*\*", r"\1", l.split("|")[3].strip())[:88]
        print(f"  {est}  {n:>3}  {titulo}")
