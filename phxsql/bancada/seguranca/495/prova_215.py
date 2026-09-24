#!/usr/bin/env python3
"""O 215 LIGADO bloqueia o IP por SQL legitimo de outro dialeto sem ';'?
Textos tirados do corpus legitimo do repositorio (bancada/gaps-sql/sondar.py).
Servidor proprio, porta propria, derrubado pelo PID; base apagada no fim."""
import json, os, shutil, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import injecao as inj
porta = int(os.environ.get("PORTA", "6797"))
textos = [l for l in (json.loads(x) for x in open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "legitimo.jsonl")))
          if l["origem"].startswith("bancada/gaps-sql/sondar.py:") and l["sql"] in (
              "SELECT * FROM clientes LIMIT 0, 2",
              "INSERT INTO clientes (nome) VALUES ('Zeca') RETURNING id",
              "SELECT nome FROM clientes EXCEPT SELECT nome FROM clientes")]
try:
    with inj.Servidor("prova215", porta, seguranca={"contar_injecao_sql": True}) as sv:
        inj.semear(sv)
        for t in textos:
            print("texto:", t["sql"], " <-", t["origem"])
        n = 0
        for rodada in range(2):
            for t in textos:
                r = sv.solto(op="sql", database="loja", texto=t["sql"])
                n += 1
                print(f"  pedido {n}: {r[:110]}")
        print("bloqueios:", json.dumps(sv.bloqueios(), ensure_ascii=False)[:300])
        print("ping depois:", sv.solto(op="ping")[:120])
finally:
    shutil.rmtree(inj.BASE, ignore_errors=True)
