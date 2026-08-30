# Exercise the server protocol end to end
# 27/08 18:45

import socket, json

def pedir(*pedidos):
    s = socket.create_connection(("127.0.0.1", 5000), timeout=5)
    f = s.makefile("rwb")
    saidas = []
    for p in pedidos:
        f.write((json.dumps(p) + "\n").encode()); f.flush()
        saidas.append(json.loads(f.readline().decode()))
    s.close()
    return saidas

T = "segredo-de-teste"
casos = [
    {"token": T, "op": "ping"},
    {"token": "ERRADO", "op": "ping"},
    {"token": T, "op": "bancos"},
    {"token": T, "op": "tabelas", "database": "Z"},
    {"token": T, "op": "buscar", "database": "Z", "tabela": "cadastroClientes",
     "indice": "porNome", "chave": "adriano boller"},
    {"token": T, "op": "inserir", "database": "Z", "tabela": "cadastroClientes",
     "valores": {"id": 99, "nome": "Inserido por TCP", "cidade": "Blumenau",
                 "limite": "4321.99", "cadastro": "2026-08-27",
                 "ficha": "veio pela porta 5000"}},
    {"token": T, "op": "ler", "database": "Z", "tabela": "cadastroClientes", "rowid": 6},
    {"token": T, "op": "diario", "database": "Z", "tabela": "cadastroClientes", "max": 3},
    {"token": T, "op": "verificar", "database": "Z", "tabela": "cadastroClientes"},
    {"token": T, "op": "ips"},
]
for r in pedir(*casos):
    print(json.dumps(r, ensure_ascii=False)[:320])
