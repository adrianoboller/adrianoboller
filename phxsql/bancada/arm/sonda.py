import json, os, socket, sys, time
# A sonda serve as duas provas de portabilidade -- a do ARM sob qemu e a do
# Windows sob wine -- porque o trabalho provado e o mesmo. So o rotulo muda,
# e ele vem de fora: sonda que se anuncia "ARM64" numa corrida de Windows
# mente no lugar que mais importa, que e o VEREDITO.
ALVO = os.environ.get("ALVO", "ARM64")
MODO = os.environ.get("MODO", "sob emulacao")
cfg = json.load(open("config.json"))
tok = cfg.get("token", "")
s = socket.create_connection(("127.0.0.1", int(os.environ.get("PORTA", "6992"))), timeout=30)
f = s.makefile("rw")
def call(d):
    d.setdefault("token", tok)
    s.sendall((json.dumps(d) + "\n").encode())
    return json.loads(f.readline())
print("  ping:", call({"op": "ping"}).get("ok"))
r=call({"op":"login","usuario":"adm","senha":"segredo1"})
print("  login:", r.get("ok"), "" if r.get("ok") else json.dumps(r)[:160])
d = call({"op": "criar_database", "database": "iot"})
print("  criar_database:", d.get("ok"), "" if d.get("ok") else json.dumps(d)[:160])
r = call({"op": "criar_tabela", "database": "iot", "tabela": "leituras",
          "colunas": [{"nome": "sensor", "tipo": "Str(32)"},
                      {"nome": "valor", "tipo": "Decimal(10,2)"}]})
print("  criar_tabela:", r.get("ok"), "" if r.get("ok") else json.dumps(r)[:180])
t0 = time.time(); n = 0
for i in range(50):
    r = call({"op": "inserir", "database": "iot", "tabela": "leituras",
              "valores": {"sensor": "s%d" % (i % 5), "valor": "%.2f" % (20 + i * 0.5)}})
    if r.get("ok"): n += 1
    elif i == 0: print("  inserir:", json.dumps(r)[:200])
dt = (time.time() - t0) * 1000
print("  inseridas: %d de 50, em %.0f ms %s (%.1f ms/linha)" % (n, dt, MODO, dt / 50))
q = call({"op": "varrer", "database": "iot", "tabela": "leituras", "limite": 1000})
# O `varrer` responde dentro de "resultado", com a contagem separada do que
# devolveu -- e e a contagem que fecha a prova: 50 gravadas, 50 devolvidas.
res = q.get("resultado", {})
lidas = res.get("devolvidas", 0)
print("  registros na tabela:", res.get("registros"), "| devolvidas:", lidas)
linhas = next((v for v in res.values() if isinstance(v, list)), [])
if linhas:
    print("  primeira:", json.dumps(linhas[0], ensure_ascii=False)[:140])
falhou = not (q.get("ok") and res.get("registros") == 50 and lidas == 50 and n == 50)
print("VEREDITO:", "REPROVOU" if falhou else "o binario %s gravou e leu 50 linhas" % ALVO)
sys.exit(1 if falhou else 0)
