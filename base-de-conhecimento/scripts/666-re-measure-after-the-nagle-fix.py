# Re-measure after the Nagle fix
# 28/08 18:40

import json, socket, time
s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
def op(p):
    p.setdefault("token","prova")
    t=time.time(); f.write((json.dumps(p)+"\n").encode()); f.flush()
    r=json.loads(f.readline().decode())
    return r.get("resultado", r), r.get("ms"), (time.time()-t)*1000
op({"op":"login","usuario":"adm","senha":"segredo1"})
r,sms,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200})
print(f"pagina 1 (posicao) ..... rede {ms:6.1f} ms | servidor {sms} ms")
cur = r["cursor_fim"]
for i in range(2,6):
    r,sms,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":cur})
    cur = r["cursor_fim"]
    print(f"pagina {i} (cursor) ...... rede {ms:6.1f} ms | servidor {sms} ms")
r,sms,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":19000})
print(f"cursor no rowid 19000 .. rede {ms:6.1f} ms | servidor {sms} ms")
r,sms,ms = op({"op":"varrer","database":"loja","tabela":"clientes","max":200,"pular":19000})
print(f"o mesmo por POSICAO .... rede {ms:6.1f} ms | servidor {sms} ms")
