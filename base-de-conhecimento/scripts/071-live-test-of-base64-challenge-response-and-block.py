# Live test of base64, challenge-response and blocking
# 27/08 19:28

import socket, json, hashlib, hmac, base64, os

def sessao():
    s=socket.create_connection(("127.0.0.1",5002),timeout=30)
    return s, s.makefile("rwb")

def pede(f, p):
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())

def m(r, corte=95):
    if r.get("ok"): return "OK   " + json.dumps(r.get("resultado"), ensure_ascii=False)[:corte]
    return "NEGA " + r.get("erro","")[:corte]

T="tok"

print("=== 1. login com Base64 (usuario e senha) ===")
s,f=sessao()
b64=lambda x: base64.b64encode(x.encode()).decode()
print("   enviado: usuario_b64=%s  senha_b64=%s" % (b64("adriano"), b64("Senha Do Adriano")))
print("  ", m(pede(f,{"token":T,"op":"login","usuario_b64":b64("adriano"),"senha_b64":b64("Senha Do Adriano")})))
print("  ", m(pede(f,{"token":T,"op":"bancos"})))
s.close()

print("\n=== 2. desafio-resposta: a senha NAO atravessa o fio ===")
s,f=sessao()
d=pede(f,{"token":T,"op":"desafio","usuario":"adriano"})["resultado"]
print("   servidor mandou: sal=%s... iteracoes=%d nonce=%s..." % (d["sal"][:12], d["iteracoes"], d["nonce"][:12]))
nc=os.urandom(16).hex()
dk=hashlib.pbkdf2_hmac("sha256", "Senha Do Adriano".encode(), bytes.fromhex(d["sal"]), d["iteracoes"], 32)
msg=f'{d["nonce"]},{nc},adriano'.encode()
prova=hmac.new(dk,msg,hashlib.sha256).hexdigest()
print("   cliente prova:  %s..." % prova[:24])
print("  ", m(pede(f,{"token":T,"op":"login","usuario":"adriano","nonce_cliente":nc,"prova":prova})))
print("  ", m(pede(f,{"token":T,"op":"ler","database":"Z","tabela":"cadastroClientes","rowid":1})))
print("   >>> nenhum byte da senha saiu da maquina do cliente")
s.close()

print("\n=== 3. desafio gravado e repetido depois (replay) ===")
s,f=sessao()
print("  ", m(pede(f,{"token":T,"op":"login","usuario":"adriano","nonce_cliente":nc,"prova":prova})))
s.close()

print("\n=== 4. comando proibido: bloqueia na hora ===")
s,f=sessao()
pede(f,{"token":T,"op":"login","usuario":"adriano","senha":"Senha Do Adriano"})
print("  ", m(pede(f,{"token":T,"op":"excluir","database":"Z","tabela":"cadastroClientes","rowid":1})))
s.close()

print("\n=== 5. o IP ja esta bloqueado: nem conecta ===")
try:
    s,f=sessao()
    print("  ", m(pede(f,{"token":T,"op":"ping"})))
    s.close()
except Exception as e:
    print("   conexao recusada:", e)
