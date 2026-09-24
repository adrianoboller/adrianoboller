#!/usr/bin/env python3
# SEC: o SEGUNDO chamador do conferir_identidade (a RESPOSTA do pulso,
# servidor.rs:3722-3735) nao confere lista nem "e este no". Um par que responde
# ao pulso (no comprometido, ou quem esta no meio de um fio sem pino) devolve
# um pulso SEM prova com id fora da lista -- e ele entra no mapa.
import json, os, socket, subprocess, threading, time, sys
R = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(R, "bin", "phxsqld")
TOK = "tok-cluster"
ID_FALSO = sys.argv[1] if len(sys.argv) > 1 else "fantasma"
def porta_livre():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p
falsa = socket.socket(); falsa.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
falsa.bind(("127.0.0.1", 0)); falsa.listen(16); pf = falsa.getsockname()[1]
vistos = []
def atender(c):
    f = c.makefile("rb")
    try:
        for l in f:
            vistos.append(l[:120])
            r = {"ok": True, "op": "cluster_pulso",
                 "resultado": {"id": ID_FALSO, "papel": "master", "epoca": 1, "posicao": 0,
                               "incompleta": False, "prioridade": 0}}
            c.sendall((json.dumps(r) + "\n").encode())
    except OSError: pass
    finally:
        f.close(); c.close()
def escutar():
    while True:
        try: c, _ = falsa.accept()
        except OSError: return
        threading.Thread(target=atender, args=(c,), daemon=True).start()
threading.Thread(target=escutar, daemon=True).start()

d = os.path.join(R, "run", "fantasma"); subprocess.run(["rm", "-rf", d]); os.makedirs(os.path.join(d, "base"))
p = porta_livre()
cfg = {"bind": f"127.0.0.1:{p}", "base": os.path.join(d, "base"), "token": TOK,
       "log_acessos": os.path.join(d, "acessos.log"), "seguranca": {"blacklist": os.path.join(d, "bl.json")},
       "dblink": os.path.join(d, "dblink.json"), "jobs": os.path.join(d, "jobs.json"), "web": {"ligado": False},
       "cifra_fio": {"ligada": True, "exigir": False, "chave_privada": "aa" * 32},
       "replicacao": {"papel": "source", "id_servidor": "noA", "imagem_da_linha": True},
       "cluster": {"id": "noA", "token": TOK, "janela_inatividade_s": 3, "pulso_s": 1, "cifra": False,
                   "exigir_prova_do_pulso": False,
                   "nos": [{"id": "noA", "endereco": "127.0.0.1", "porta": p},
                           {"id": "noB", "endereco": "127.0.0.1", "porta": pf},
                           {"id": "noC", "endereco": "127.0.0.1", "porta": porta_livre()}]}}
c = os.path.join(d, "config.json"); json.dump(cfg, open(c, "w"))
err = open(os.path.join(d, "stderr.txt"), "w")
proc = subprocess.Popen([BIN, "--config", c], stdout=err, stderr=err, cwd=d)
try:
    for _ in range(100):
        try: socket.create_connection(("127.0.0.1", p), timeout=0.2).close(); break
        except OSError: time.sleep(0.05)
    def falar(obj):
        s = socket.create_connection(("127.0.0.1", p)); s.settimeout(5)
        s.sendall((json.dumps(obj) + "\n").encode()); b = b""
        while b"\n" not in b:
            x = s.recv(65536)
            if not x: break
            b += x
        s.close(); return json.loads(b.split(b"\n")[0])
    e0 = falar({"op": "cluster_estado", "token": TOK})["resultado"]
    print(f"ANTES : papel={e0['papel']} epoca={e0['epoca']} master={e0.get('master')} escrita_liberada={e0.get('escrita_liberada')}")
    time.sleep(4.5)
    e1 = falar({"op": "cluster_estado", "token": TOK})["resultado"]
    print(f"DEPOIS: papel={e1['papel']} epoca={e1['epoca']} master={e1.get('master')} escrita_liberada={e1.get('escrita_liberada')} degradado={e1.get('degradado')}")
    print("pulsos que o par falso recebeu:", len(vistos), "| primeiro:", vistos[0] if vistos else None)
    print("stderr do no A (cluster):")
    for l in open(os.path.join(d, "stderr.txt")):
        if "cluster" in l or "REBAIX" in l: print("   ", l.rstrip()[:200])
finally:
    proc.kill(); proc.wait(); falsa.close()
