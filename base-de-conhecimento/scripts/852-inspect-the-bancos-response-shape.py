# Inspect the bancos response shape
# 28/08 22:33

import json, socket
s = socket.create_connection(("127.0.0.1", 5900)); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token", "demo")
    f.write((json.dumps(p) + "\n").encode()); f.flush()
    return json.loads(f.readline().decode())
fala({"op":"login","usuario":"adm","senha":"segredo1"})
print("bancos:", json.dumps(fala({"op":"bancos"})["resultado"])[:200])
