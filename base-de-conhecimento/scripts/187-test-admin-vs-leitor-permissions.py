# Test admin vs leitor permissions
# 27/08 21:19

import socket, json
def sessao(u):
    s=socket.create_connection(('127.0.0.1',5400),5); f=s.makefile('rwb')
    def pede(d):
        f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
    pede({"token":"tk","op":"login","usuario":u,"senha":"x"})
    return pede
for quem, esperado in [("ana","admin"), ("joao","leitor")]:
    p = sessao(quem)
    r = p({"token":"tk","op":"backup","destino":"backups","zip":True})
    l = p({"token":"tk","op":"varrer","database":"Comercial","tabela":"cadastroClientes","max":1})
    print(f'{quem:6} nivel {esperado:8} | backup: {"OK" if r["ok"] else r["erro"][:52]}')
    print(f'{"":6}                | ler   : {"OK" if l["ok"] else l["erro"][:52]}')
