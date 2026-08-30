# Check if the mark survives to the esquema response
# 29/08 18:57

import json, socket
s = socket.create_connection(("127.0.0.1", 5399)); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token","segredo"); f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())
fala({"op":"login","usuario":"adriano","senha":"demo123"})
r = fala({"op":"esquema","database":"loja","tabela":"clientes"})
res = r.get("resultado", r)
cols = res.get("colunas", [])
print("campos que a resposta traz por coluna:", sorted(cols[1].keys()) if len(cols)>1 else cols)
for c in cols[:8]:
    print(f"  {c.get('nome'):18} dado_pessoal={c.get('dado_pessoal')!r}")
