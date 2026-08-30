# Find the held segment by scene detection
# 28/08 22:27

ts = [float(x) for x in open('/tmp/mudancas.txt') if x.strip()]
maior = (0, 0, 0)
for a, b in zip(ts, ts[1:]):
    if b - a > maior[0]:
        maior = (b - a, a, b)
print(f"maior intervalo sem mudanca: {maior[0]:.1f}s  de {maior[1]:.1f}s a {maior[2]:.1f}s")
# os cinco maiores
gaps = sorted(((b-a, a, b) for a, b in zip(ts, ts[1:])), reverse=True)[:5]
for g, a, b in gaps:
    print(f"  {g:5.1f}s   {a:6.1f} -> {b:6.1f}")
