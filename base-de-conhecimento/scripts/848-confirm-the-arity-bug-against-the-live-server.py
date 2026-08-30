# Confirm the arity bug against the live server
# 28/08 22:17

import json, socket
s = socket.create_connection(("127.0.0.1", 5900)); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token", "demo")
    f.write((json.dumps(p) + "\n").encode()); f.flush()
    return json.loads(f.readline().decode())
fala({"op":"login","usuario":"adm","senha":"segredo1"})
e = fala({"op":"esquema","database":"Comercial","tabela":"cadastroClientes"})["resultado"]
print("colunas:", [(c["nome"], c.get("sistema")) for c in e["colunas"]])
# o que a ficha manda hoje: tudo menos a PRIMEIRA coluna de sistema
sistema = next((c["nome"] for c in e["colunas"] if c.get("sistema")), "softdeleted")
editaveis = [c for c in e["colunas"] if c["nome"] != sistema]
print("a ficha manda", len(editaveis), "valores para", len(e["colunas"]), "colunas")
vals = [1,"Adriano Boller","Blumenau","SC","1500.00","2024-10-04","ficha",1]
print("atualizar com 8:", fala({"op":"atualizar","database":"Comercial","tabela":"cadastroClientes",
                                "rowid":1,"valores":vals}).get("erro"))
print("atualizar com 7:", fala({"op":"atualizar","database":"Comercial","tabela":"cadastroClientes",
                                "rowid":1,"valores":vals[:7]}).get("ok"))
