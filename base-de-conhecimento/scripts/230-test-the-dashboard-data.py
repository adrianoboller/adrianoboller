# Test the dashboard data
# 27/08 22:34

import socket, json
s=socket.create_connection(('127.0.0.1',5500),5); f=s.makefile('rwb')
def pede(d):
    f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
pede({"token":"tk","op":"login","usuario":"root","senha":"x"})
r=pede({"token":"tk","op":"painel"})
if not r["ok"]: print("ERRO:", r["erro"]); raise SystemExit
d=r["resultado"]
print("resumo:", json.dumps(d["resumo"])[:340])
print("bancos:", d["bancos"])
print("maiores:", d["maiores_tabelas"])
print("por_operacao:", d["por_operacao"][:5])
print("por_nivel:", d["por_nivel"])
print("por_hora:", d["por_hora"])
