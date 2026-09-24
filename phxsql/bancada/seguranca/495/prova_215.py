#!/usr/bin/env python3
"""O 215 LIGADO bloqueia o IP por SQL legitimo de outro dialeto sem ';'?
Textos tirados do corpus legitimo do repositorio (bancada/gaps-sql/sondar.py).
Servidor proprio, porta propria, derrubado pelo PID; base apagada no fim.

Os 3 textos vao DIRETO aqui (nao por `legitimo.jsonl`): o pedido 501 pos os
mesmos 3 textos como teste em `comando_empilhado_nao_acusa_o_legitimo`
(sintaxe.rs), e o extrator dedupica por TEXTO -- quem varre primeiro (Rust,
antes de Python na ordem do proprio extrator) fica com a origem, entao o
filtro por `origem.startswith("bancada/gaps-sql/sondar.py:")` passou a achar
zero linhas depois do conserto. Os textos continuam sendo os mesmos do
`sondar.py:229,251,254` -- so a fonte de leitura mudou."""
import json, os, shutil, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import injecao as inj
porta = int(os.environ.get("PORTA", "6797"))
textos = [
    {"sql": "SELECT * FROM clientes LIMIT 0, 2", "origem": "bancada/gaps-sql/sondar.py:251"},
    {
        "sql": "INSERT INTO clientes (nome) VALUES ('Zeca') RETURNING id",
        "origem": "bancada/gaps-sql/sondar.py:229",
    },
    {
        "sql": "SELECT nome FROM clientes EXCEPT SELECT nome FROM clientes",
        "origem": "bancada/gaps-sql/sondar.py:254",
    },
]
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
