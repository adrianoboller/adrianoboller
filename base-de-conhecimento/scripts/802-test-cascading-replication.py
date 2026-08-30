# Test cascading replication
# 28/08 20:25

import time, sys; sys.path.insert(0, ".")
from conferir import liga, retrato
m, s1, s3 = liga(5800), liga(5801), liga(5803)
def pos(f): return f({"op":"posicao","database":"loja"})["resultado"]["tabelas"]["clientes"]["eventos"]
print("antes:", {"master": pos(m), "slave01": pos(s1), "slave03": pos(s3)})
linhas = [{"id": 3_000_000 + k, "nome": f"Cascata {k}", "cidade": "Bruxelas",
           "limite": "42.00", "ficha": f"em cascata {k}"} for k in range(500)]
m({"op":"inserir_lote","database":"loja","tabela":"clientes","linhas":linhas})
alvo = pos(m)
t0 = time.perf_counter()
while time.perf_counter() - t0 < 60:
    if pos(s3) >= alvo: break
    time.sleep(0.05)
print(f"depois: master {alvo} | slave01 {pos(s1)} | slave03 {pos(s3)}  em {time.perf_counter()-t0:.1f}s")
for n, f in [("master", m), ("slave01", s1), ("slave03", s3)]:
    l, h = retrato(f); print(f"  {n:<8} {l} linhas  retrato {h}")
