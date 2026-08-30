# Test the profiler against the live server
# 28/08 22:57

import json, socket, time
def liga(porta=5900):
    s = socket.create_connection(("127.0.0.1", porta)); f = s.makefile("rwb")
    def fala(p):
        p.setdefault("token", "demo")
        f.write((json.dumps(p) + "\n").encode()); f.flush()
        return json.loads(f.readline().decode())
    return fala

adm = liga(); adm({"op":"login","usuario":"adm","senha":"segredo1"})
LOG = "/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/logs/monitor.txt"
print("ligar:", adm({"op":"profiler_ligar","arquivo":LOG,"guardar":50})["resultado"])

# trafego de outra conexao, inclusive um LOGIN com senha
outro = liga()
outro({"op":"login","usuario":"adm","senha":"segredo1"})
outro({"op":"varrer","database":"Comercial","tabela":"cadastroClientes","max":3})
outro({"op":"inserir","database":"Comercial","tabela":"cadastroClientes",
       "linha":{"id":8888888,"nome":"Pelo profiler","cidade":"Blumenau","uf":"SC","limite":"12.34"}})
outro({"op":"inserir","database":"Comercial","tabela":"cadastroClientes",
       "linha":{"id":8888888,"nome":"repetido","cidade":"X"}})   # chave duplicada
time.sleep(0.3)

r = adm({"op":"profiler","max":10})["resultado"]
print(f"\nligado={r['ligado']} observados={r['observados']} arquivo={r['arquivo']}")
print(f"{'op':<14}{'usuario':<8}{'ms':>4}  ok     pedido")
for e in r["eventos"]:
    print(f"{e['op']:<14}{e['usuario']:<8}{str(e['ms']):>4}  {str(e['ok']):<6} {e['pedido'][:88]}")
print("\n--- o arquivo em disco ---")
print(open(LOG).read())
print("VAZOU A SENHA?", "SIM" if "segredo1" in open(LOG).read() else "nao")
