#!/usr/bin/env python3
"""Exercita os dois conteineres: bases diferentes, tabelas diferentes, e o
canal direto entre eles."""
import json, socket, sys, time

TOKEN = "token-do-teste-dois-docker"
SENHA = "segredo-do-teste"

class Cliente:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta), 8)
        self.f = self.s.makefile("rwb")
    def call(self, o):
        o.setdefault("token", TOKEN)
        self.f.write((json.dumps(o) + "\n").encode()); self.f.flush()
        return json.loads(self.f.readline().decode())
    def entrar(self):
        return self.call({"op": "login", "usuario": "adm", "senha": SENHA})
    def fechar(self):
        self.f.close(); self.s.close()

def ok(r, oq):
    if not r.get("ok"):
        print(f"   ERRO em {oq}: {r.get('erro')}")
    return r.get("ok")

def tabela(c, db, tab, colunas):
    c.call({"op": "criar_database", "database": db})
    return c.call({"op": "criar_tabela", "database": db, "tabela": tab,
                   "colunas": colunas,
                   "indices": [{"nome": "pk", "colunas": [colunas[0]["nome"]],
                                "unico": True, "primario": True}]})

COL_ID = {"nome": "id", "tipo": "Int8", "obrigatoria": True}

a, b = Cliente(6500), Cliente(6510)
print("login phx-a:", a.entrar().get("ok"), " · login phx-b:", b.entrar().get("ok"))

print("\n== 1. bases e tabelas DIFERENTES em cada um ==")
print("-- phx-a: base `loja`")
ok(tabela(a, "loja", "clientes", [COL_ID, {"nome": "nome", "tipo": "Str(40)"},
                                  {"nome": "cidade", "tipo": "Str(30)"}]), "loja.clientes")
ok(tabela(a, "loja", "pedidos", [COL_ID, {"nome": "cliente_id", "tipo": "Int8"},
                                 {"nome": "total", "tipo": "Decimal(12,2)"}]), "loja.pedidos")
print("-- phx-b: base `rh`")
ok(tabela(b, "rh", "funcionarios", [COL_ID, {"nome": "nome", "tipo": "Str(40)"},
                                    {"nome": "cargo_id", "tipo": "Int8"}]), "rh.funcionarios")
ok(tabela(b, "rh", "cargos", [COL_ID, {"nome": "titulo", "tipo": "Str(30)"}]), "rh.cargos")

print("\n== 2. dado em cada um ==")
for i, (n, ci) in enumerate([("Ana Prado", "Blumenau"), ("Bruno Reis", "Joinville"),
                             ("Carla Nunes", "Curitiba")], 1):
    a.call({"op": "inserir", "database": "loja", "tabela": "clientes",
            "linha": {"id": i, "nome": n, "cidade": ci}})
for i, t in enumerate(["Analista", "Gerente"], 1):
    b.call({"op": "inserir", "database": "rh", "tabela": "cargos",
            "linha": {"id": i, "titulo": t}})

def conta(c, db, tab):
    r = c.call({"op": "varrer", "database": db, "tabela": tab, "max": 100})
    return len(r["resultado"]["linhas"]) if r.get("ok") else f"ERRO {r.get('erro')[:60]}"

print(f"   phx-a  loja.clientes ..... {conta(a,'loja','clientes')}")
print(f"   phx-b  rh.cargos ......... {conta(b,'rh','cargos')}")

print("\n== 3. cada um enxerga SO a propria base? ==")
print("   phx-a bancos:", a.call({"op": "bancos"}).get("resultado"))
print("   phx-b bancos:", b.call({"op": "bancos"}).get("resultado"))

print("\n== 4. o canal DIRETO: phx-b puxa de phx-a pela rede do docker ==")
for tentativa in range(1, 13):
    time.sleep(2.5)
    bancos = b.call({"op": "bancos"}).get("resultado") or []
    if "loja" in bancos:
        print(f"   chegou na tentativa {tentativa} (~{tentativa*2.5:.0f}s)")
        break
else:
    print("   NAO chegou em 30 s")
print("   phx-b bancos agora:", b.call({"op": "bancos"}).get("resultado"))
print(f"   phx-b  loja.clientes .... {conta(b,'loja','clientes')}   (veio de phx-a)")
print(f"   phx-b  rh.cargos ........ {conta(b,'rh','cargos')}   (nasceu aqui)")
a.fechar(); b.fechar()
