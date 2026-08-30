# Generate activity so the dashboard has data
# 27/08 22:37

import socket, json, random
for quem in ["root"]*3:
    s=socket.create_connection(('127.0.0.1',5500),5); f=s.makefile('rwb')
    def pede(d):
        f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
    pede({"token":"tk","op":"login","usuario":quem,"senha":"x"})
    D={"token":"tk","database":"Comercial","tabela":"cadastroClientes"}
    for _ in range(12): pede({**D,"op":"varrer","max":5})
    for _ in range(7):  pede({**D,"op":"esquema"})
    for _ in range(5):  pede({**D,"op":"ler","rowid":1})
    for _ in range(3):  pede({**D,"op":"diario"})
    pede({"token":"tk","op":"memoria_carregar", **{k:v for k,v in D.items() if k!="token"}})
# alguns recusados, de um IP com token errado
for _ in range(4):
    s=socket.create_connection(('127.0.0.1',5500),5); f=s.makefile('rwb')
    f.write(b'{"token":"errado","op":"ping"}\n'); f.flush(); f.readline()
print("movimento gerado")
