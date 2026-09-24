#!/usr/bin/env python3
"""Ida-e-volta de um pedido `sql` pelo soquete, para pôr o custo do detector em
escala. Mede do lado do cliente Python (inclui o proprio Python: o denominador
e MAIOR que o custo do servidor, e isso favorece o detector -- dito no texto)."""
import os, shutil, sys, time, statistics
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import injecao as inj
porta = int(os.environ.get("PORTA", "6799"))
N = 2000
try:
    with inj.Servidor("rtt", porta) as sv:
        inj.semear(sv)
        rodadas = []
        for sql in ("SELECT nome FROM clientes WHERE nome = 'Alves'", "SELECT * FROM clientes WHERE nome = '' OR cidade = 'x'"):
            for _ in range(200):
                sv.cru(op="sql", database="loja", texto=sql)
            ts = []
            for _ in range(5):
                t = time.perf_counter()
                for _ in range(N):
                    sv.cru(op="sql", database="loja", texto=sql)
                ts.append((time.perf_counter() - t) / N * 1e6)
            print(f"{sql[:60]:60} us/pedido min/med/max: {min(ts):.1f} / {statistics.median(ts):.1f} / {max(ts):.1f}")
        t = time.perf_counter()
        for _ in range(N):
            sv.cru(op="ping")
        print(f"{'ping (piso do fio + Python)':60} us/pedido: {(time.perf_counter()-t)/N*1e6:.1f}")
finally:
    shutil.rmtree(inj.BASE, ignore_errors=True)
