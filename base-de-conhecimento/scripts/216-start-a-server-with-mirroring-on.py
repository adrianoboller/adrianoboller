# Start a server with mirroring on
# 27/08 21:47

import socket, json, os, time
S="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/espelho/srv"
time.sleep(2)
s=socket.create_connection(('127.0.0.1',5500),5); f=s.makefile('rwb')
def pede(d):
    f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
pede({"token":"tk","op":"login","usuario":"root","senha":"x"})
D={"token":"tk","database":"Comercial","tabela":"cadastroClientes"}
print("2. ler pelo servidor:", pede({**D,"op":"ler","rowid":1})["ok"])
print("   arquivos agora   :", sorted(x for x in os.listdir(f"{S}/dados/Comercial")))
