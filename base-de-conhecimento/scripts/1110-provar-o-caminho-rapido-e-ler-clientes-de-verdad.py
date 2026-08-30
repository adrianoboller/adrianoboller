# Provar o caminho rapido e ler clientes de verdade
# 29/08 11:12

import json, socket
s = socket.create_connection(("127.0.0.1", 5599)); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token","segredo")
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())
assert fala({"op":"login","usuario":"adriano","senha":"demo123"}).get("ok")

r = fala({"op":"dblink_testar","nome":"crm"})["resultado"]
print("caminho rapido do caching_sha2:", r["versao"], "| usuario efetivo:", r["usuario_efetivo"], "| base:", r["database"], f"| {r['ms']} ms")

r = fala({"op":"dblink_tabelas","nome":"crm"})["resultado"]
print("tabelas do outro lado:", [(t["nome"], t["motor"], t["registros_estimados"], t["comentario"] or "-") for t in r["tabelas"]])

r = fala({"op":"dblink_ler","nome":"crm","tabela":"clientes","ordem":"id"})["resultado"]
print("colunas:", [c for c in r["colunas"]])
for l in r["linhas"]: print("  ", l)

r = fala({"op":"dblink_estrutura","nome":"crm","tabela":"clientes"})["resultado"]
print("indices:", [(i[2], i[4]) for i in r["indices"]["linhas"]])
