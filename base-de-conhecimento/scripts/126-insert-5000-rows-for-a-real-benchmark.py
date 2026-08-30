# Insert 5000 rows for a real benchmark
# 27/08 20:26

import socket, json, time
s = socket.create_connection(('127.0.0.1', 5200), 5)
f = s.makefile('rwb')
def pede(d):
    f.write((json.dumps(d)+'\n').encode()); f.flush()
    return json.loads(f.readline().decode())
T='token-fw'
import base64
print('login:', pede({"token":T,"op":"login","usuario":"root","senha":"Boller@2026"})["ok"])
D={"token":T,"database":"Comercial","tabela":"cadastroClientes"}
cidades=["Blumenau","Joinville","Itajai","Curitiba","Chapeco","Lages"]
t0=time.time()
N=5000
for i in range(7, 7+N):
    r=pede({**D,"op":"inserir","valores":{"id":i,"nome":f"Cliente {i:05d}",
            "cidade":cidades[i%len(cidades)],"limite":f"{(i%9000)+100}.00","cadastro":"2026-08-27"}})
    if not r["ok"]: print("FALHOU:", r); break
print(f'inseriu {N} linhas em {time.time()-t0:.1f}s')
print('recarrega em memoria:', pede({**D,"op":"memoria_carregar"})["resultado"])
