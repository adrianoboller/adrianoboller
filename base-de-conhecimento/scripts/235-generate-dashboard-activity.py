# Generate dashboard activity
# 27/08 22:37

import socket, json
def sess():
    s=socket.create_connection(('127.0.0.1',5500),5); f=s.makefile('rwb')
    def pede(d):
        f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
    return pede
p = sess()
print("login:", p({"token":"tk","op":"login","usuario":"root","senha":"x"})["ok"])
D={"token":"tk","database":"Comercial","tabela":"cadastroClientes"}
for _ in range(12): p({**D,"op":"varrer","max":5})
for _ in range(7):  p({**D,"op":"esquema"})
for _ in range(5):  p({**D,"op":"ler","rowid":1})
for _ in range(3):  p({**D,"op":"diario"})
p({**D,"op":"memoria_carregar"})
for _ in range(6):  p({**D,"op":"SelectMemory","onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"}]})
print("movimento gerado")
