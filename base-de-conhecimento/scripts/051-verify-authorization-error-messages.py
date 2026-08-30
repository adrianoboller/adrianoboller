# Verify authorization error messages
# 27/08 19:08

import socket, json
s=socket.create_connection(("127.0.0.1",5001),timeout=20); f=s.makefile("rwb")
for p in [{"token":"tok","op":"bancos"},
          {"token":"errado","op":"ping"},
          {"token":"tok","op":"login","usuario":"carlos","senha":"troque-esta-senha"},
          {"token":"tok","op":"inserir","database":"Z","tabela":"cadastroClientes","valores":{"id":1}}]:
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    r=json.loads(f.readline().decode())
    print(f"  {r.get('erro') or 'OK'}")
s.close()
