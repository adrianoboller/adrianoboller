# Seed 20k rows for the UI test
# 28/08 18:37

import json, socket, time
def fala(pedidos):
    s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
    out=[]
    for p in pedidos:
        p.setdefault("token","prova")
        f.write((json.dumps(p)+"\n").encode()); f.flush()
        out.append(json.loads(f.readline().decode()))
    s.close(); return out

base=[{"op":"login","usuario":"adm","senha":"segredo1"},
      {"op":"criar_database","database":"loja"},
      {"op":"criar_tabela","database":"loja","tabela":"clientes",
       "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":True},
                  {"nome":"nome","tipo":"Str(40)","obrigatoria":True},
                  {"nome":"cidade","tipo":"Str(30)"}],
       "indices":[{"nome":"porId","colunas":["id"],"unico":True,"primario":True}]}]
for r in fala(base):
    if not r.get("ok",True): print("ERRO:", r)

# 20 mil linhas, em lote
t0=time.time()
s = socket.create_connection(("127.0.0.1", 5741)); f=s.makefile("rwb")
f.write((json.dumps({"op":"login","token":"prova","usuario":"adm","senha":"segredo1"})+"\n").encode()); f.flush(); f.readline()
for i in range(1, 20001):
    p={"op":"inserir","token":"prova","database":"loja","tabela":"clientes",
       "linha":{"id":i,"nome":f"Cliente {i:05}","cidade":"Blumenau" if i%3 else "Itajai"},
       "gravar":"lote"}
    f.write((json.dumps(p)+"\n").encode())
    if i % 500 == 0:
        f.flush()
        for _ in range(500): f.readline()
f.flush()
for _ in range(20000 % 500): f.readline()
s.close()
print(f"20.000 linhas em {time.time()-t0:.1f}s")
