# Verify the cascade and measure the two-hop lag
# 28/08 20:28

import time, sys; sys.path.insert(0, ".")
from conferir import liga, retrato, PORTAS
C = {n: liga(p) for n, p in PORTAS.items()}
def pos(f): return f({"op":"posicao","database":"loja"})["resultado"]["tabelas"]["clientes"]["eventos"]

print("topologia: master -> slave01 -> slave03 ; master -> slave02")
print("posicao:", {n: pos(f) for n, f in C.items()})
for n, f in C.items():
    l, h = retrato(f); print(f"  {n:<8} {l:>6} linhas  retrato {h}")

# Uma escrita, e o tempo ate cada um -- o de segundo salto paga dois lacos.
alvo_antes = pos(C["master"])
C["master"]({"op":"inserir","database":"loja","tabela":"clientes",
             "linha":{"id":7777777,"nome":"Cascata","cidade":"Bruxelas",
                      "limite":"1.00","ficha":"linha de prova da cascata"}})
alvo = pos(C["master"])
t0 = time.perf_counter(); chegou = {}
while time.perf_counter() - t0 < 60 and len(chegou) < 3:
    for n in ("slave01", "slave02", "slave03"):
        if n not in chegou and pos(C[n]) >= alvo:
            chegou[n] = (time.perf_counter() - t0) * 1000
    time.sleep(0.02)
print()
for n in ("slave01", "slave02", "slave03"):
    salto = "2 saltos (pelo slave01)" if n == "slave03" else "1 salto"
    print(f"  {n}: {chegou.get(n, float('nan')):.0f} ms  ({salto})")
print()
for n, f in C.items():
    l, h = retrato(f); print(f"  {n:<8} {l:>6} linhas  retrato {h}")
