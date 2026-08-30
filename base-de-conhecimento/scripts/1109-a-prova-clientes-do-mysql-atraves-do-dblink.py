# A prova: clientes do MySQL atraves do DbLink
# 29/08 11:12

import json, socket

s = socket.create_connection(("127.0.0.1", 5599))
f = s.makefile("rwb")
def fala(p):
    p.setdefault("token", "segredo")
    f.write((json.dumps(p) + "\n").encode()); f.flush()
    return json.loads(f.readline().decode())

r = fala({"op":"login","usuario":"adriano","senha":"demo123"})
assert r.get("ok"), r
print("login ok")

r = fala({"op":"dblink_salvar","nome":"crm","motor":"mysql","host":"127.0.0.1",
          "porta":3306,"usuario":"phx","senha":"ponte123","database":"crm",
          "descricao":"o MySQL(R) da bancada, banco crm"})
print("salvar:", json.dumps(r.get("resultado", r), ensure_ascii=False)[:200])

r = fala({"op":"dblink_testar","nome":"crm"})
print("testar:", json.dumps(r.get("resultado", r), ensure_ascii=False))

r = fala({"op":"dblink_tabelas","nome":"crm"})
res = r.get("resultado", r)
print("tabelas:", [(t.get("nome"), t.get("motor"), t.get("registros_estimados")) for t in res.get("tabelas", [])])

r = fala({"op":"dblink_ler","nome":"crm","tabela":"clientes","ordem":"id"})
res = r.get("resultado", r)
print("colunas:", res.get("colunas"))
for l in res.get("linhas", []):
    print("  ", l)
