#!/usr/bin/env python3
"""So o login por prova (desafio-resposta), intercalado, prova errada.

    python3 bancada/seguranca/520-521/sonda-prova.py BINARIO ROTULO [N]

Compara `ana` (existe, ativa) com `zzz` (nao existe, mesmo tamanho de login)
e com `ze` (existe, inativo). Mede so o pedido do login, depois do desafio.
"""
import os, statistics, sys

AQUI = os.path.dirname(os.path.abspath(__file__))
N = int(sys.argv[3]) if len(sys.argv) > 3 else 1000
sys.argv = [sys.argv[0], sys.argv[1], sys.argv[2] + "-prova", "--rapido"]
fonte = open(os.path.join(AQUI, "sonda-520.py")).read().split("\nsrv = subir()")[0]
g = {"__file__": os.path.join(AQUI, "sonda-520.py")}
exec(compile(fonte, "sonda-520.py", "exec"), g)
srv = g["subir"]()
try:
    for rodada in range(2):
        c = g["Con"]()
        logins = ["ana", "zzz", "ze"]
        t = {u: [] for u in logins}
        for i in range(N):
            ordem = logins[i % 3:] + logins[:i % 3]
            for u in ordem:
                c.pedir({"op": "desafio", "usuario": u})
                dt, r = c.pedir({"op": "login", "usuario": u, "prova": "00" * 32, "nonce_cliente": "abc"})
                t[u].append(dt)
        c.fechar()
        m = {u: statistics.median(v) * 1e6 for u, v in t.items()}
        print(f"rodada {rodada}: mediana existe {m['ana']:.1f} us, nao existe {m['zzz']:.1f} us "
              f"({m['zzz'] - m['ana']:+.1f}), inativo {m['ze']:.1f} us ({m['ze'] - m['ana']:+.1f})  (n={N})")
finally:
    srv.terminate()
    srv.wait(timeout=10)
