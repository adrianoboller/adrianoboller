"""Sobe um phxsqld de verdade numa porta da faixa 6250-6299 e conversa por
soquete. Mata so o PID que subiu -- nunca pkill."""
import json
import os
import shutil
import signal
import socket
import subprocess
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ",
                      os.path.abspath(os.path.join(AQUI, "..", "..")))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
TOKEN = "token-de-servico"


def hash_da_senha(s):
    saida = subprocess.run([PHXSQLD, "--senha"], input=s + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def config_padrao(porta, web=False):
    return {
        "base": "base", "bind": "127.0.0.1:%d" % porta, "token": TOKEN,
        "web": {"ligado": bool(web), "bind": "127.0.0.1:%d" % (porta + 1)},
        "usuarios": [
            {"login": "adm", "nome": "Adriano", "id": 10, "nivel": "admin",
             "senha_hash": hash_da_senha("senha-do-adm"),
             "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                             "excluir": True, "criar": True,
                             "administrar": True, "verificar": True}}},
            # Leitor com `administrar` na regra "*": NAO e admin do servidor,
            # mas o portao geral pergunta pela base VAZIA e a regra "*" diz sim.
            {"login": "curioso", "nome": "Curioso", "id": 11, "nivel": "leitor",
             "senha_hash": hash_da_senha("senha-do-curioso"),
             "bases": {"loja": {"ler": True}, "*": {"administrar": True}}},
            {"login": "leitor", "nome": "Leitor", "id": 12, "nivel": "leitor",
             "senha_hash": hash_da_senha("senha-do-leitor"),
             "bases": {"*": {"ler": True}}},
        ],
    }


def subir(base, porta, config=None, limpar=True, binario=None):
    if limpar:
        shutil.rmtree(base, ignore_errors=True)
    os.makedirs(base, exist_ok=True)
    with open(os.path.join(base, "config.json"), "w") as f:
        json.dump(config or config_padrao(porta), f, indent=2)
    saida = open(os.path.join(base, "servidor.log"), "a")
    p = subprocess.Popen([binario or PHXSQLD], cwd=base, stdout=saida,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(80):
        time.sleep(0.25)
        try:
            socket.create_connection(("127.0.0.1", porta), 0.4).close()
            return p
        except OSError:
            continue
    raise SystemExit("o servidor nao subiu na porta %d" % porta)


def baixar(p):
    p.send_signal(signal.SIGTERM)
    try:
        p.wait(10)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait(5)


class Conexao:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta))
        self.f = self.s.makefile("rwb")

    def cru(self, texto):
        self.f.write(texto.encode() + b"\n")
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def fala(self, p):
        p.setdefault("token", TOKEN)
        return self.cru(json.dumps(p))

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p["op"], r.get("erro")))
        return r["resultado"]

    def entrar(self, login, senha):
        return self.ok({"op": "login", "usuario": login, "senha": senha})

    def fechar(self):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass
