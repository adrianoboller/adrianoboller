# Compute the exact insert ratios
# 29/08 01:28

import json
d = json.load(open('bancada/resultados.json'))
ins = {r['motor']: r for r in d if r['fase']=='inserir'}
p, m = ins['PhxSql'], ins['MySQL']
n = p['operacoes']
print(f"linhas: {n:,}".replace(",","."))
print(f"PhxSql: {p['segundos']:.2f}s  {n/p['segundos']:,.0f}/s  {p['segundos']*1e6/n:.1f} us/linha  cpu {p['cpu_s']:.0f}s  escrito {p['escrito_mb']:,.0f} MiB")
print(f"MySQL : {m['segundos']:.2f}s  {n/m['segundos']:,.0f}/s  {m['segundos']*1e6/n:.1f} us/linha  cpu {m['cpu_s']:.0f}s  escrito {m['escrito_mb']:,.0f} MiB")
print()
print(f"tempo:      {p['segundos']/m['segundos']:.3f}x  -> {100*(p['segundos']/m['segundos']-1):.1f}% a MAIS de tempo")
print(f"vazao:      {(n/p['segundos'])/(n/m['segundos'])*100:.1f}% da vazao do MySQL")
print(f"            {100-((n/p['segundos'])/(n/m['segundos'])*100):.1f}% a MENOS de vazao")
