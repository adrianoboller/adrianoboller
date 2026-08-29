#!/usr/bin/env python3
"""Monta o banco da prova do driver ODBC: a tabela `loja.clientes` com INT,
VARCHAR, DECIMAL e DATE, e linhas CONHECIDAS -- a prova-abi.py compara o que
o driver devolve com o que este script gravou, valor por valor.

Roda contra um phxsqld SEU (ver docs/ODBC.md, secao da prova). Uso:

    python3 montar-dados.py [host porta token usuario senha]
"""
import json, socket, sys

HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORTA = int(sys.argv[2]) if len(sys.argv) > 2 else 5305
TOKEN = sys.argv[3] if len(sys.argv) > 3 else "prova-odbc"
USUARIO = sys.argv[4] if len(sys.argv) > 4 else "root"
SENHA = sys.argv[5] if len(sys.argv) > 5 else "prova123"

s = socket.create_connection((HOST, PORTA))
f = s.makefile("rwb")

def fala(p, tolera=False):
    p.setdefault("token", TOKEN)
    f.write((json.dumps(p) + "\n").encode()); f.flush()
    r = json.loads(f.readline().decode())
    if not r.get("ok") and not tolera:
        sys.exit(f"FALHOU {p['op']}: {r.get('erro')}")
    return r

fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
# `tolera` nas duas criacoes: rodar de novo sobre um banco ja montado nao e
# erro -- e o caso comum de quem esta iterando na prova.
fala({"op": "criar_database", "database": "loja"}, tolera=True)
fala({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
      "colunas": [
          {"nome": "id",     "tipo": "Int4",          "obrigatoria": True},
          {"nome": "nome",   "tipo": "Str(40)"},
          {"nome": "limite", "tipo": "Decimal(12,2)"},
          {"nome": "desde",  "tipo": "Date"},
      ],
      "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]}, tolera=True)

# As mesmas linhas que a prova-abi.py espera, na mesma ordem. O limite NULO
# da linha 3 e proposital: e o que prova o indicador SQL_NULL_DATA.
LINHAS = [
    {"id": 1, "nome": "Adriano Boller", "limite": "15000.00", "desde": "2019-03-12"},
    {"id": 2, "nome": "Maria Operadora", "limite": "4200.50", "desde": "2021-07-01"},
    {"id": 3, "nome": "Carlos Consulta", "limite": None, "desde": "2024-12-25"},
]
for linha in LINHAS:
    # `tolera` porque rodar duas vezes esbarra no indice unico -- e ai a
    # conferencia final de contagem e quem decide se o estado serve.
    fala({"op": "inserir", "database": "loja", "tabela": "clientes",
          "valores": linha}, tolera=True)

r = fala({"op": "sql", "database": "loja",
          "texto": "SELECT COUNT(*) FROM clientes"})
n = r["resultado"]["contagem"]
print(f"linhas na tabela: {n}")
if n != 3:
    sys.exit(f"esperava 3 linhas e ha {n} -- a tabela tinha dado de antes? "
             "Apague dados/loja e rode de novo.")
