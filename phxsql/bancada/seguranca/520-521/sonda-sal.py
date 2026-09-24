#!/usr/bin/env python3
"""Quem tem o token calcula o sal falso do desafio e sabe quem NAO existe.

    python3 bancada/seguranca/520-521/sonda-sal.py BINARIO ROTULO
"""
import hashlib, hmac, os, sys

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.argv = [sys.argv[0], sys.argv[1], sys.argv[2] + "-sal", "--rapido"]
fonte = open(os.path.join(AQUI, "sonda-520.py")).read().split("\nsrv = subir()")[0]
g = {"__file__": os.path.join(AQUI, "sonda-520.py")}
exec(compile(fonte, "sonda-520.py", "exec"), g)
srv = g["subir"]()
try:
    c = g["Con"]()
    for u in ["ana", "ze", "root", "zzz", "nao_existe", "fulano"]:
        _, r = c.pedir({"op": "desafio", "usuario": u})
        sal = r["resultado"]["sal"]
        previsto = hmac.new(g["TOKEN"].encode(), u.encode(), hashlib.sha256).hexdigest()[:32]
        print(f"{u:12s} sal {sal}  == HMAC(token, login)[:16]? {'SIM -> nao existe' if sal == previsto else 'nao -> existe'}")
    c.fechar()
finally:
    srv.terminate()
    srv.wait(timeout=10)
