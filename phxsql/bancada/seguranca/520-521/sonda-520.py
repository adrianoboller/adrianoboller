#!/usr/bin/env python3
"""Sonda dos pedidos 520 e 521 pelo soquete, contra um binario dado.

    python3 bancada/seguranca/520-521/sonda-520.py BINARIO ROTULO [--rapido]

Mede, na porta de dados e na porta web, o tempo de cada caminho que responde
«credencial invalida»: senha errada de quem existe, de quem esta inativo e de
quem nao existe; a prova do desafio-resposta nos mesmos tres; o proprio
desafio; e o custo pelo tamanho da senha (521). Binario DEBUG: o absoluto e de
debug, o que vale e a razao. Com ROTULO `antes`, pula os casos que num binario
sem o conserto levariam horas.

Os temporarios vivem em `/tmp/phx-520-<pid>` e saem no fim; o servidor e
derrubado pelo PID que esta sonda guardou, nunca por nome. As outras tres
sondas desta pasta reusam as funcoes daqui.
"""
import atexit, hashlib, http.client, json, os, shutil, socket, statistics, subprocess, sys, time

AQUI = os.path.dirname(os.path.abspath(__file__))
BIN = sys.argv[1]
ROTULO = sys.argv[2]
RAPIDO = "--rapido" in sys.argv
BASE = f"/tmp/phx-520-{os.getpid()}"
atexit.register(shutil.rmtree, BASE, True)
PORTA = int(os.environ.get("PHX_520_PORTA", "47731"))
PORTA_WEB = PORTA + 1
TOKEN = "t-sonda-520"


def hash_de(senha, sal=b"0123456789abcdef", it=210000):
    dk = hashlib.pbkdf2_hmac("sha256", senha.encode(), sal, it, 32)
    return f"pbkdf2-sha256${it}${sal.hex()}${dk.hex()}"


def subir():
    subprocess.run(["rm", "-rf", BASE])
    os.makedirs(os.path.join(BASE, "base"))
    cfg = {
        "bind": f"127.0.0.1:{PORTA}", "token": TOKEN, "base": os.path.join(BASE, "base"),
        "log_acessos": os.path.join(BASE, "acessos.log"),
        "seguranca": {"blacklist": os.path.join(BASE, "blacklist.json"), "whitelist": ["127.0.0.1"]},
        "dblink": os.path.join(BASE, "dblink.json"), "jobs": os.path.join(BASE, "jobs.json"),
        "root": {"login": "root", "senha_hash": hash_de("senha-certa-da-sonda")},
        "usuarios": [
            {"login": "ana", "senha_hash": hash_de("senha-da-ana", b"fedcba9876543210"), "nivel": "leitor"},
            {"login": "ze", "senha_hash": hash_de("senha-do-ze", b"a1b2c3d4e5f60718"), "nivel": "leitor", "ativo": False},
        ],
        "web": {"ligado": True, "bind": f"127.0.0.1:{PORTA_WEB}"},
        "cifra_fio": {"exigir": False},
    }
    cfgp = os.path.join(BASE, "config.json")
    json.dump(cfg, open(cfgp, "w"))
    srv = subprocess.Popen([BIN, "--config", cfgp], stdout=open(os.path.join(BASE, "out.txt"), "w"),
                           stderr=subprocess.STDOUT)
    ate = time.time() + 30
    while True:
        try:
            socket.create_connection(("127.0.0.1", PORTA), timeout=2).close()
            socket.create_connection(("127.0.0.1", PORTA_WEB), timeout=2).close()
            return srv
        except OSError:
            if time.time() > ate:
                srv.terminate()
                raise
            time.sleep(0.1)


class Con:
    def __init__(self, prazo=900):
        self.s = socket.create_connection(("127.0.0.1", PORTA), timeout=prazo)
        self.buf = b""

    def pedir(self, obj):
        obj = dict(obj, token=TOKEN)
        self.s.sendall((json.dumps(obj) + "\n").encode())
        t0 = time.perf_counter()
        while b"\n" not in self.buf:
            k = self.s.recv(65536)
            if not k:
                return time.perf_counter() - t0, {"ok": False, "erro": "<fim da conexao>"}
            self.buf += k
        linha, self.buf = self.buf.split(b"\n", 1)
        return time.perf_counter() - t0, json.loads(linha)

    def fechar(self):
        self.s.close()


def web(obj, prazo=900):
    c = http.client.HTTPConnection("127.0.0.1", PORTA_WEB, timeout=prazo)
    corpo = json.dumps(dict(obj, token=TOKEN))
    t0 = time.perf_counter()
    c.request("POST", "/api", body=corpo, headers={"Content-Type": "application/json"})
    r = c.getresponse()
    dados = r.read()
    dt = time.perf_counter() - t0
    c.close()
    try:
        return dt, json.loads(dados)
    except Exception:
        return dt, {"ok": False, "erro": dados[:80].decode(errors="replace")}


def resumo(xs):
    xs = sorted(xs)
    return f"mediana {statistics.median(xs)*1e3:10.3f} ms  min {xs[0]*1e3:10.3f}  max {xs[-1]*1e3:10.3f}  n={len(xs)}"


def curto(r):
    return (r.get("erro") or json.dumps(r.get("resultado")))[:90]


srv = subir()
try:
    print(f"== {ROTULO}: {BIN}")
    # 1. login por senha: os quatro que respondem «credencial invalida»
    n = 2 if RAPIDO else 3
    for rot, u in [("existe, senha errada", "root"), ("existe, ativo=false", "ze"), ("nao existe", "nao_existe")]:
        c = Con()
        ts = []
        for _ in range(n):
            dt, r = c.pedir({"op": "login", "usuario": u, "senha": "x" * 8})
            ts.append(dt)
        c.fechar()
        print(f"senha  {rot:22s} {resumo(ts)}  {curto(r)}")
    # 2. login por prova (desafio-resposta), com e sem amarracao: a prova e lixo
    n = 200 if not RAPIDO else 50
    for rot, u in [("existe", "ana"), ("existe, ativo=false", "ze"), ("nao existe", "nao_existe")]:
        c = Con()
        ts = []
        for _ in range(n):
            c.pedir({"op": "desafio", "usuario": u})
            dt, r = c.pedir({"op": "login", "usuario": u, "prova": "00" * 32, "nonce_cliente": "abc"})
            ts.append(dt)
        c.fechar()
        print(f"prova  {rot:22s} {resumo(ts)}  {curto(r)}")
    # 3. o proprio desafio
    for rot, u in [("existe", "ana"), ("nao existe", "nao_existe")]:
        c = Con()
        ts = []
        for _ in range(n):
            dt, r = c.pedir({"op": "desafio", "usuario": u})
            ts.append(dt)
        c.fechar()
        print(f"desafio {rot:21s} {resumo(ts)}  iteracoes={r.get('resultado', {}).get('iteracoes')}")
    # 4. 521: o custo pelo tamanho da senha, porta de dados
    for u, tam in [("root", 8), ("root", 1024), ("nao_existe", 1024), ("root", 60000)]:
        if tam >= 60000 and (RAPIDO or ROTULO == "antes"):
            continue
        c = Con(prazo=1200)
        try:
            dt, r = c.pedir({"op": "login", "usuario": u, "senha": "x" * tam})
        except socket.timeout:
            dt, r = 1200, {"erro": "<prazo>"}
        c.fechar()
        print(f"tamanho {u:10s} {tam:7d} B  {dt*1e3:12.1f} ms  {curto(r)}")
    # 5. 521 pela porta WEB, antes da credencial: o corpo aceita 4 MiB
    for tam in [8, 1024, 70000, 1_000_000]:
        if ROTULO == "antes" and tam > 1024:
            print(f"web     root       {tam:7d} B  (nao medido no antes: 1 MiB x 210.000 iteracoes nao termina)")
            continue
        try:
            dt, r = web({"op": "login", "usuario": "root", "senha": "x" * tam}, prazo=1200)
        except socket.timeout:
            dt, r = 1200, {"erro": "<prazo>"}
        print(f"web     root       {tam:7d} B  {dt*1e3:12.1f} ms  {curto(r)}")
    # 6. depois da credencial: CREATE USER com senha grande
    c = Con(prazo=1200)
    dt, r = c.pedir({"op": "login", "usuario": "root", "senha": "senha-certa-da-sonda"})
    print(f"root entrou: {r.get('ok')} em {dt*1e3:.1f} ms")
    for tam in [16, 65535, 65536, 1 << 20]:
        if ROTULO == "antes" and tam > 16:
            print(f"criar   {tam:8d} B  (nao medido no antes: o SEC mediu >300 s com 1 MiB)")
            continue
        try:
            dt, r = c.pedir({"op": "sql", "texto": f"CREATE USER u{tam} PASSWORD '{'y' * tam}'"})
        except socket.timeout:
            dt, r = 1200, {"erro": "<prazo>"}
        print(f"criar   {tam:8d} B  {dt*1e3:12.1f} ms  {curto(r)}")
    c.fechar()
finally:
    srv.terminate()
    srv.wait(timeout=10)
    log = open(os.path.join(BASE, "out.txt")).read()
    vazou = [s for s in ("x" * 64, "y" * 64) if s in log]
    print(f"stderr do servidor: {len(log)} bytes; senha no log: {'SIM' if vazou else 'nao'}")
