# Tabulate every bench phase
# 29/08 22:30

import json
d=json.load(open('phxsql/bancada/resultados.json'))
print(f"{'fase':12} {'motor':8} {'ops':>10} {'segundos':>10} {'por_segundo':>14}")
for r in d:
    ops=r.get("operacoes",0); s=r.get("segundos",0)
    ps = ops/s if s else 0
    print(f"{r.get('fase',''):12} {r.get('motor',''):8} {ops:>10} {s:>10.3f} {ps:>14,.0f}")
