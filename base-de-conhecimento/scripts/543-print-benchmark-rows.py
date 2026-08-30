# Print benchmark rows
# 28/08 17:16

import json,io
for r in json.load(io.open('resultados.json',encoding='utf-8')):
    print(f"{r['motor']:>8} {r['fase']:<18} {r['segundos']:>10.3f}s cpu={r['cpu_s']:>8.2f} esc={r['escrito_mb']:>10.1f}MB lid={r['lido_mb']:>9.1f}MB rss={r['pico_rss_mb']:>7.1f} ops={r['operacoes']}")
