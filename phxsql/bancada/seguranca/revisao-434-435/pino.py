#!/usr/bin/env python3
# SEC 435: o bit «X tem pino aqui?» pelo ramo SEM prova (resposta de SUCESSO).
# Sobe um no A como o `subir_no_a` do teste identidade-do-pulso.rs: noB COM
# pino, noC SEM pino, exigir_prova_do_pulso desligado (fabrica).
import json, os, socket, subprocess, time
R = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(R, "bin", "phxsqld")
PUB_B = "6b0b616d718e53691236d3be3ce6d44f9d28836426d81305d131f488206f8d2b"
TOK = "tok-cluster"
def porta_livre():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p
d = os.path.join(R, "run", "pino"); subprocess.run(["rm", "-rf", d]); os.makedirs(os.path.join(d, "base"))
p = porta_livre()
cfg = {"bind": f"127.0.0.1:{p}", "base": os.path.join(d, "base"), "token": TOK,
       "log_acessos": os.path.join(d, "acessos.log"), "seguranca": {"blacklist": os.path.join(d, "bl.json")},
       "dblink": os.path.join(d, "dblink.json"), "jobs": os.path.join(d, "jobs.json"), "web": {"ligado": False},
       "cifra_fio": {"ligada": True, "exigir": False, "chave_privada": "aa" * 32},
       "replicacao": {"papel": "source", "id_servidor": "noA", "imagem_da_linha": True},
       "cluster": {"id": "noA", "token": TOK, "janela_inatividade_s": 3, "pulso_s": 1, "cifra": False,
                   "exigir_prova_do_pulso": False,
                   "nos": [{"id": "noA", "endereco": "127.0.0.1", "porta": p},
                           {"id": "noB", "endereco": "127.0.0.1", "porta": 7498, "chave_do_fio": PUB_B},
                           {"id": "noC", "endereco": "127.0.0.1", "porta": 7499}]}}
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
        s.close(); return b.split(b"\n")[0].decode()
    # A SONDA: pulso SEM prova, com posicao que o `registrar` descarta (>= 2^53):
    # o veredito e a resposta voltam inteiros, e o mapa do cluster nao muda.
    for no in ("noB", "noC"):
        r = falar({"op": "cluster_pulso", "token": TOK, "id": no, "papel": "replica",
                   "epoca": 0, "posicao": 1e16, "incompleta": False, "prioridade": 0})
        j = json.loads(r); res = j.get("resultado", {})
        print(f"sonda sem prova como {no}: ok={j.get('ok')} bytes={len(r)} campos={sorted(res.keys())}")
    estado = falar({"op": "cluster_estado", "token": TOK})
    print("cluster_estado:", estado)
    time.sleep(0.2)
    print("stderr do no A:")
    for l in open(os.path.join(d, "stderr.txt")):
        if "cluster" in l: print("   ", l.rstrip()[:200])
finally:
    proc.kill(); proc.wait()
