#!/usr/bin/env python3
"""Mede a premissa do SEC: «as classes classicas nao executam, viram dado/erro».
So usa o que o repositorio ja tem: o Servidor/semear e o ARSENAL de
bancada/seguranca/injecao.py. Servidor proprio, porta propria, derrubado pelo PID."""
import json, os, shutil, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import injecao as inj

def linhas(r):
    def achar(o):
        if isinstance(o, dict):
            for k, v in o.items():
                if k == "linhas" and isinstance(v, list):
                    return v
                x = achar(v)
                if x is not None:
                    return x
        return None
    return achar(r)

porta = int(os.environ.get("PORTA", "6795"))
arsenal = dict(inj.ARSENAL)
casos = [
    ("controle: nome = '' (vazio, legitimo)", "SELECT * FROM clientes WHERE nome = ''"),
    ("ARSENAL aspa solta / tautologia", arsenal["aspa solta / tautologia"]),
    ("ARSENAL aspa escapada como DADO", arsenal["aspa escapada como DADO"]),
    ("controle: sem WHERE (todas)", "SELECT * FROM clientes"),
]
try:
    with inj.Servidor("premissa", porta) as sv:
        inj.semear(sv)
        for rot, sql in casos:
            r = json.loads(sv.cru(op="sql", database="loja", texto=sql))
            l = linhas(r)
            op = (r.get("resultado") or {}).get("op")
            print(f"{rot:42} ok={r.get('ok')!s:5} op={op!s:8} linhas={len(l) if l is not None else '-'}"
                  + ("" if r.get("ok") else f"  erro={str(r.get('erro'))[:90]}"))
finally:
    shutil.rmtree(inj.BASE, ignore_errors=True)
