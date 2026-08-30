# Provar tambem o caminho nativo
# 29/08 11:12

import json, socket
s = socket.create_connection(("127.0.0.1", 5599)); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token","segredo")
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())
assert fala({"op":"login","usuario":"adriano","senha":"demo123"}).get("ok")
r = fala({"op":"dblink_testar","nome":"crm"})["resultado"]
print("mysql_native_password:", r["ok"], "| versao", r["versao"], "| usuario efetivo:", r["usuario_efetivo"])
