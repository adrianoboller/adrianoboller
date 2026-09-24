#!/usr/bin/env python3
"""So o desafio, intercalado: existe x nao existe, N rodadas, na mesma conexao.

    python3 bancada/seguranca/520-521/sonda-desafio.py BINARIO ROTULO [N] [LOGIN_FALSO]
"""
import os, statistics, sys

AQUI = os.path.dirname(os.path.abspath(__file__))
N = int(sys.argv[3]) if len(sys.argv) > 3 else 2000
# `zzz` tem o tamanho de `ana`: com `nao_existe` sobram ~2 us de debug que sao
# o tamanho do login (analise do JSON, linha do log), e nao o cadastro.
FALSO = sys.argv[4] if len(sys.argv) > 4 else "zzz"
sys.argv = [sys.argv[0], sys.argv[1], sys.argv[2] + "-desafio", "--rapido"]
# Reusa so as funcoes da sonda grande, sem rodar o roteiro dela.
fonte = open(os.path.join(AQUI, "sonda-520.py")).read().split("\nsrv = subir()")[0]
g = {"__file__": os.path.join(AQUI, "sonda-520.py")}
exec(compile(fonte, "sonda-520.py", "exec"), g)
srv = g["subir"]()
try:
    for rodada in range(2):
        c = g["Con"]()
        t = {"ana": [], FALSO: []}
        for i in range(N):
            for u in (("ana", FALSO) if i % 2 else (FALSO, "ana")):
                dt, r = c.pedir({"op": "desafio", "usuario": u})
                t[u].append(dt)
        c.fechar()
        m = {u: statistics.median(v) * 1e6 for u, v in t.items()}
        q = {u: statistics.quantiles(v, n=4) for u, v in t.items()}
        print(f"rodada {rodada}: mediana existe {m['ana']:.1f} us, nao existe {m[FALSO]:.1f} us, "
              f"diferenca {m[FALSO] - m['ana']:+.1f} us; "
              f"q1/q3 existe {q['ana'][0]*1e6:.1f}/{q['ana'][2]*1e6:.1f}, "
              f"nao existe {q[FALSO][0]*1e6:.1f}/{q[FALSO][2]*1e6:.1f}  (n={N})")
finally:
    srv.terminate()
    srv.wait(timeout=10)
