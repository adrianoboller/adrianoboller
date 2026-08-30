# Test that admin can back up and leitor cannot
# 27/08 21:18

import socket, json
def sessao(u):
    s=socket.create_connection(('127.0.0.1',5400),5); f=s.makefile('rwb')
    def pede(d):
        f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
    pede({"token":"tk","op":"login","usuario":u,"senha":"x"})
    return pede
for quem, esperado in [("ana","admin: deve poder"), ("joao","leitor: NAO deve poder")]:
    p = sessao(quem)
    r = p({"token":"tk","op":"backup","destino":"backups","zip":True})
    print(f'{quem:6} ({esperado:24}) -> ', end='')
    print('OK' if r["ok"] else r["erro"])
    l = p({"token":"tk","op":"varrer","database":"Comercial","tabela":"cadastroClientes","max":1})
    print(f'{"":6}  ler                        -> ', 'OK' if l["ok"] else l["erro"])
