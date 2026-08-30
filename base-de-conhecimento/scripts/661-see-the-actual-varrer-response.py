# See the actual varrer response
# 28/08 18:39

import json, socket, time
s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
def op(p):
    p.setdefault("token","prova")
    t=time.time(); f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode()), (time.time()-t)*1000
print(op({"op":"login","usuario":"adm","senha":"segredo1"})[0].get("ok"))
r,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200})
print(json.dumps({k:v for k,v in r.items() if k!="linhas"})[:400])
