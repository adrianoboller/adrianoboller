# Live test of authentication and permissions
# 27/08 19:07

import socket, json
def sessao(*pedidos):
    s=socket.create_connection(("127.0.0.1",5001),timeout=30); f=s.makefile("rwb"); out=[]
    for p in pedidos:
        f.write((json.dumps(p)+"\n").encode()); f.flush()
        out.append(json.loads(f.readline().decode()))
    s.close(); return out
T="tok"
def m(r):
    if r.get("ok"): 
        res=r.get("resultado")
        return "OK   " + (json.dumps(res,ensure_ascii=False)[:110] if res is not None else "")
    return "NEGA " + r.get("erro","")

print("--- sem login, com token valido ---")
for r in sessao({"token":T,"op":"ping"},{"token":T,"op":"bancos"}):
    print(" ", m(r))

print("\n--- login errado ---")
for r in sessao({"token":T,"op":"login","usuario":"maria","senha":"chute"},
                {"token":T,"op":"login","usuario":"nao-existe","senha":"x"}):
    print(" ", m(r))

print("\n--- maria: ler+inserir+alterar em Z, sem excluir ---")
ped=[{"token":T,"op":"login","usuario":"maria","senha":"troque-esta-senha"},
     {"token":T,"op":"bancos","database":"Z"},
     {"token":T,"op":"inserir","database":"Z","tabela":"cadastroClientes",
      "valores":{"id":77,"nome":"Inserido pela Maria","cidade":"Itajai"}},
     {"token":T,"op":"excluir","database":"Z","tabela":"cadastroClientes","rowid":6},
     {"token":T,"op":"ips","database":"Z"},
     {"token":T,"op":"diario","database":"Z","tabela":"cadastroClientes","max":2}]
for p,r in zip(ped,sessao(*ped)):
    print(f"  {p['op']:<10} {m(r)}")

print("\n--- carlos: so leitura em Z, e nada fora de Z ---")
ped=[{"token":T,"op":"login","usuario":"carlos","senha":"troque-esta-senha"},
     {"token":T,"op":"varrer","database":"Z","tabela":"cadastroClientes","max":1},
     {"token":T,"op":"inserir","database":"Z","tabela":"cadastroClientes","valores":{"id":1}},
     {"token":T,"op":"tabelas","database":"W"}]
for p,r in zip(ped,sessao(*ped)):
    print(f"  {p['op']:<10} {m(r)}")

print("\n--- root: pode tudo ---")
ped=[{"token":T,"op":"login","usuario":"root","senha":"troque-esta-senha-do-root"},
     {"token":T,"op":"quem_sou"},
     {"token":T,"op":"excluir","database":"Z","tabela":"cadastroClientes","rowid":6},
     {"token":T,"op":"usuarios"}]
for p,r in zip(ped,sessao(*ped)):
    print(f"  {p['op']:<10} {m(r)}")
