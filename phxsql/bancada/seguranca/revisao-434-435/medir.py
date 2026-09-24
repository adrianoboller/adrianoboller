#!/usr/bin/env python3
# Medicao SEC 434 (revisao adversaria): custo da drenagem, identidade velha,
# corrida do cadastro vazio e retencao da linha anterior. Pelo soquete, contra
# o binario de release (copia em bin/). Sobe e derruba os PROPRIOS servidores.
import json, os, socket, subprocess, sys, threading, time

R = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(R, "bin", "phxsqld")
H = "pbkdf2-sha256$210000$f7c13cae04930580f6e5cfca27d8caf0$551ea4ed38e905b1cb3a783d2a235e97ee54372e3f3b6a4302443f1af25cd3dd"
SENHA = "segredo-sec"
TOK = "tok"
KIB = 1024; MIB = 1024 * 1024
TETO_APERTO = 64 * KIB; TETO_REG = 128 * MIB

def porta_livre():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p

def subir(nome, com_cadastro, extra_usuarios=True):
    d = os.path.join(R, "run", nome)
    subprocess.run(["rm", "-rf", d]); os.makedirs(os.path.join(d, "base"))
    p = porta_livre()
    cfg = {"bind": f"127.0.0.1:{p}", "base": os.path.join(d, "base"), "token": TOK,
           "log_acessos": os.path.join(d, "acessos.log"), "blacklist": os.path.join(d, "blacklist.json"),
           "dblink": os.path.join(d, "dblink.json"), "jobs": os.path.join(d, "jobs.json"),
           "timeout_s": 60, "cifra_fio": {"exigir": False}, "web": {"ligado": False}}
    if com_cadastro:
        cfg["root"] = {"login": "root", "senha_hash": H}
        if extra_usuarios:
            cfg["usuarios"] = [{"login": "ana", "senha_hash": H, "nivel": "leitor"}]
    caminho = os.path.join(d, "config.json")
    with open(caminho, "w") as f: json.dump(cfg, f)
    err = open(os.path.join(d, "stderr.txt"), "w")
    proc = subprocess.Popen([BIN, "--config", caminho], stdout=err, stderr=err, cwd=d)
    ate = time.time() + 10
    while time.time() < ate:
        try:
            socket.create_connection(("127.0.0.1", p), timeout=0.2).close(); return proc, p, d
        except OSError: time.sleep(0.05)
    proc.kill(); raise SystemExit(f"servidor {nome} nao subiu")

def ticks(pid):
    c = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
    return int(c[11]) + int(c[12])  # utime+stime (campos 14 e 15)

def status(pid, k):
    for l in open(f"/proc/{pid}/status"):
        if l.startswith(k): return int(l.split()[1])  # kB
    return -1

class Lig:
    def __init__(s, p):
        s.s = socket.create_connection(("127.0.0.1", p)); s.s.settimeout(120); s.buf = b""
    def linha(s):
        while b"\n" not in s.buf:
            b = s.s.recv(65536)
            if not b: return None
            s.buf += b
        l, s.buf = s.buf.split(b"\n", 1); return json.loads(l)
    def mandar(s, obj_ou_txt):
        t = obj_ou_txt if isinstance(obj_ou_txt, str) else json.dumps(obj_ou_txt)
        s.s.sendall(t.encode() + b"\n"); return s.linha()
    def fechar(s): s.s.close()

def ping_gordo(n):
    return '{"token":"%s","op":"ping","enchimento":"%s"}' % (TOK, "A" * n)

def curto(r):
    if r is None: return "(conexao fechou sem resposta)"
    return {k: r.get(k) for k in ("ok", "op", "nome", "codigo") if k in r} | ({"erro": r["erro"][:110]} if "erro" in r else {})

HZ = os.sysconf("SC_CLK_TCK")

def t1_drenagem(n_con):
    proc, p, d = subir(f"t1-{n_con}", True)
    try:
        time.sleep(0.3)
        c0 = ticks(proc.pid); rss0 = status(proc.pid, "VmRSS")
        total = TETO_APERTO + 1 + TETO_REG
        bloco = b"A" * (4 * MIB)
        pico = [status(proc.pid, "VmRSS")]; parar = [False]
        def amostrar():
            while not parar[0]:
                pico.append(status(proc.pid, "VmRSS")); time.sleep(0.02)
        am = threading.Thread(target=amostrar); am.start()
        res = [None] * n_con
        def um(i):
            s = socket.create_connection(("127.0.0.1", p)); s.settimeout(300)
            falta = total
            try:
                while falta > 0:
                    k = min(falta, len(bloco)); s.sendall(bloco[:k]); falta -= k
                buf = b""
                while b"\n" not in buf:
                    b = s.recv(65536)
                    if not b: break
                    buf += b
                res[i] = (total - falta, buf.split(b"\n")[0][:160])
            except OSError as e:
                res[i] = (total - falta, f"ERRO {e}".encode())
            s.close()
        t0 = time.time()
        th = [threading.Thread(target=um, args=(i,)) for i in range(n_con)]
        [t.start() for t in th]; [t.join() for t in th]
        parede = time.time() - t0
        parar[0] = True; am.join()
        c1 = ticks(proc.pid)
        cpu = (c1 - c0) / HZ
        respostas = sum(1 for r in res if r and b"limite excedido" in r[1])
        gib = n_con * total / (1024 ** 3)
        print(f"T1 conexoes={n_con} bytes/conexao={total} total={gib:.2f} GiB parede={parede:.2f}s "
              f"cpu_servidor={cpu:.2f}s ({cpu/gib:.2f} s/GiB) rss_antes={rss0}kB rss_pico={max(pico)}kB "
              f"vmhwm={status(proc.pid,'VmHWM')}kB respostas_LIMITE={respostas}/{n_con}")
        print("   exemplo:", res[0][1][:150])
        log = open(os.path.join(d, "acessos.log")).read().strip().splitlines()
        print("   acessos.log linhas:", len(log), "| ultima:", log[-1][:220] if log else "-")
    finally:
        proc.kill(); proc.wait()

def t2_identidade_velha():
    proc, p, d = subir("t2", True)
    try:
        c1 = Lig(p); r = c1.mandar({"token": TOK, "op": "login", "usuario": "ana", "senha": SENHA})
        print("T2 login ana:", curto(r))
        c2 = Lig(p); r = c2.mandar({"token": TOK, "op": "login", "usuario": "root", "senha": SENHA})
        print("   login root:", curto(r))
        r = c2.mandar({"token": TOK, "op": "usuario_excluir", "login": "ana"})
        print("   root exclui ana:", curto(r))
        # A ficha de ana so cai no proximo despachar de C1; o teto da proxima
        # linha de C1 JA foi escolhido (128 MiB) antes da exclusao.
        r = c1.mandar(ping_gordo(1 * MIB))
        print("   C1 (ana EXCLUIDA) manda 1 MiB  ->", curto(r))
        r = c1.mandar(ping_gordo(1 * MIB))
        print("   C1 manda 1 MiB de novo          ->", curto(r))
        c3 = Lig(p); r = c3.mandar(ping_gordo(1 * MIB))
        print("   C3 anonima nova manda 1 MiB     ->", curto(r))
    finally:
        proc.kill(); proc.wait()

def t3_corrida_cadastro_vazio():
    proc, p, d = subir("t3", False)
    try:
        c1 = Lig(p)  # anonima, parada no ler_ate com o teto do servidor SEM cadastro
        time.sleep(0.2)
        c2 = Lig(p); r = c2.mandar({"token": TOK, "op": "usuario_criar", "login": "adm", "senha": SENHA, "supervisor": True})
        print("T3 anonima cria o primeiro usuario:", curto(r))
        r = c1.mandar(ping_gordo(1 * MIB))
        print("   C1 (aberta ANTES do cadastro) manda 1 MiB ->", curto(r))
        c4 = Lig(p); r = c4.mandar(ping_gordo(1 * MIB))
        print("   C4 anonima nova manda 1 MiB             ->", curto(r))
    finally:
        proc.kill(); proc.wait()

def t4_retencao():
    proc, p, d = subir("t4", True)
    try:
        c = Lig(p); r = c.mandar({"token": TOK, "op": "login", "usuario": "root", "senha": SENHA}); print("T4 login root:", curto(r))
        time.sleep(0.3); a = status(proc.pid, "VmRSS")
        r = c.mandar(ping_gordo(64 * MIB)); print("   root manda ping de 64 MiB ->", curto(r))
        time.sleep(1.0); b = status(proc.pid, "VmRSS")
        r = c.mandar({"token": TOK, "op": "ping"}); time.sleep(1.0); cc = status(proc.pid, "VmRSS")
        print(f"   VmRSS: antes={a}kB | respondida e OCIOSA={b}kB | depois de uma linha pequena={cc}kB")
    finally:
        proc.kill(); proc.wait()

if __name__ == "__main__":
    qual = sys.argv[1:] or ["t2", "t3", "t4", "t1-1", "t1-64"]
    for q in qual:
        if q == "t2": t2_identidade_velha()
        elif q == "t3": t3_corrida_cadastro_vazio()
        elif q == "t4": t4_retencao()
        elif q.startswith("t1-"): t1_drenagem(int(q[3:]))

def t5_linhas_vazias(n_con=4, mib=8):
    """Q2: custo por linha VAZIA -- anonima (toma a trava do cadastro no
    teto_da_linha) contra logada (nao toma). As duas pulam o despachar."""
    for modo in ("anonima", "logada"):
        proc, p, d = subir(f"t5-{modo}", True)
        try:
            ligs = []
            for i in range(n_con):
                c = Lig(p)
                if modo == "logada":
                    r = c.mandar({"token": TOK, "op": "login", "usuario": "root", "senha": SENHA})
                    assert r.get("ok"), r
                ligs.append(c)
            time.sleep(0.3)
            bloco = b"\n" * MIB
            c0 = ticks(proc.pid); t0 = time.time()
            def um(c):
                for _ in range(mib): c.s.sendall(bloco)
                r = c.mandar({"token": TOK, "op": "ping"})  # sincroniza: tudo lido
                assert r and r.get("ok"), r
            th = [threading.Thread(target=um, args=(c,)) for c in ligs]
            [t.start() for t in th]; [t.join() for t in th]
            parede = time.time() - t0; cpu = (ticks(proc.pid) - c0) / HZ
            linhas = n_con * mib * MIB
            print(f"T5 {modo}: {linhas/1e6:.1f} M linhas vazias em {n_con} conexoes | parede={parede:.2f}s "
                  f"cpu_servidor={cpu:.2f}s -> {cpu/linhas*1e9:.0f} ns/linha")
        finally:
            proc.kill(); proc.wait()

if __name__ == "__main__" and sys.argv[1:] == ["t5"]:
    t5_linhas_vazias()

if __name__ == "__main__" and sys.argv[1:] == ["t5b"]:
    import itertools
    def rodar(modo, n_con, mib):
        proc, p, d = subir(f"t5b-{modo}-{n_con}", True)
        try:
            ligs = []
            for i in range(n_con):
                c = Lig(p)
                if modo == "logada":
                    assert c.mandar({"token": TOK, "op": "login", "usuario": "root", "senha": SENHA}).get("ok")
                ligs.append(c)
            time.sleep(0.3); bloco = b"\n" * MIB
            c0 = ticks(proc.pid); t0 = time.time()
            def um(c):
                for _ in range(mib): c.s.sendall(bloco)
                assert c.mandar({"token": TOK, "op": "ping"}).get("ok")
            th = [threading.Thread(target=um, args=(c,)) for c in ligs]
            [t.start() for t in th]; [t.join() for t in th]
            parede = time.time() - t0; cpu = (ticks(proc.pid) - c0) / HZ; linhas = n_con * mib * MIB
            print(f"T5b {modo:8s} conexoes={n_con} linhas={linhas/1e6:.1f}M parede={parede:.2f}s cpu={cpu:.2f}s -> {cpu/linhas*1e9:.0f} ns/linha  (load {open('/proc/loadavg').read().split()[0]})")
        finally:
            proc.kill(); proc.wait()
    for modo, n in [("logada", 4), ("anonima", 4), ("logada", 1), ("anonima", 1), ("anonima", 4), ("logada", 4)]:
        rodar(modo, n, 4)

if __name__ == "__main__" and sys.argv[1:] == ["t5c"]:
    # Comparacao: linha NAO vazia minima ("x\n") de anonimo -- vai ao despachar,
    # responde e anota no acessos.log. E o pior caso PRE-EXISTENTE por linha.
    n = 50_000
    proc, p, d = subir("t5c", True)
    try:
        s = socket.create_connection(("127.0.0.1", p)); s.settimeout(60)
        lidos = [0]
        def ler():
            try:
                while True:
                    b = s.recv(1 << 20)
                    if not b: break
                    lidos[0] += b.count(b"\n")
                    if lidos[0] >= n: break
            except OSError: pass
        t = threading.Thread(target=ler); t.start()
        c0 = ticks(proc.pid); t0 = time.time()
        s.sendall(b"x\n" * n); t.join()
        parede = time.time() - t0; cpu = (ticks(proc.pid) - c0) / HZ
        tam = os.path.getsize(os.path.join(d, "acessos.log"))
        print(f"T5c anonima 'x\\n': {n} linhas, respostas={lidos[0]} parede={parede:.2f}s cpu={cpu:.2f}s -> {cpu/n*1e9:.0f} ns/linha, acessos.log={tam/n:.0f} B/linha")
    finally:
        proc.kill(); proc.wait(); subprocess.run(["rm", "-rf", d])
