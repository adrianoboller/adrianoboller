# Time the server side of paging
# 28/08 18:38

import json, socket, time
s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
def op(p):
    p.setdefault("token","prova")
    t=time.time(); f.write((json.dumps(p)+"\n").encode()); f.flush()
    r=json.loads(f.readline().decode()); return r, (time.time()-t)*1000
op({"op":"login","usuario":"adm","senha":"segredo1"})

r,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200})
print(f"pagina 1 (posicao) ..... {ms:6.1f} ms  ms_servidor={r.get('ms')}  devolvidas={r['devolvidas']}")
cur = r["cursor_fim"]
for i in range(2,7):
    r,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":cur})
    cur = r["cursor_fim"]
    print(f"pagina {i} (cursor) ...... {ms:6.1f} ms  ms_servidor={r.get('ms')}")
# uma pagina bem no fim
r,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":19000})
print(f"pagina no rowid 19000 .. {ms:6.1f} ms  ms_servidor={r.get('ms')}")
# e o mesmo ponto por POSICAO
r,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200,"pular":19000})
print(f"o mesmo por posicao .... {ms:6.1f} ms  ms_servidor={r.get('ms')}")
r,ms = op({"op":"esquema","database":"loja","tabela":"clientes"})
print(f"esquema ................ {ms:6.1f} ms  ms_servidor={r.get('ms')}")
