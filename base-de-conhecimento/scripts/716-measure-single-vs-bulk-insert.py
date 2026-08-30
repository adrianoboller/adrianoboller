# Measure single vs bulk insert
# 28/08 19:21

import json, socket, time
s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
def op(p):
    p.setdefault("token","prova")
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())
op({"op":"login","usuario":"adm","senha":"segredo1"})
op({"op":"criar_database","database":"loja"})
cols=[{"nome":"id","tipo":"Int4","obrigatoria":True},
      {"nome":"nome","tipo":"Str(40)","obrigatoria":True},
      {"nome":"cidade","tipo":"Str(30)"}]
for t in ("uma_a_uma","em_lote"):
    op({"op":"criar_tabela","database":"loja","tabela":t,"colunas":cols,
        "indices":[{"nome":"porId","colunas":["id"],"unico":True,"primario":True}]})

N=20000
linha=lambda i:{"id":i,"nome":f"Cliente {i:05}","cidade":"Blumenau" if i%3 else "Itajai"}

t0=time.time()
for i in range(1,N+1):
    op({"op":"inserir","database":"loja","tabela":"uma_a_uma","linha":linha(i)})
uma=time.time()-t0
print(f"uma a uma ... {uma:7.2f} s  ({N/uma:8.0f} linhas/s)")

t0=time.time()
gravadas=0
for bloco in range(0, N, 1000):
    r=op({"op":"inserir_lote","database":"loja","tabela":"em_lote",
          "linhas":[linha(i) for i in range(bloco+1, bloco+1001)]})
    gravadas += r["resultado"]["gravadas"]
lote=time.time()-t0
print(f"em lote ..... {lote:7.2f} s  ({N/lote:8.0f} linhas/s)   gravadas={gravadas}")
print(f"ganho ....... {uma/lote:.1f}x")
