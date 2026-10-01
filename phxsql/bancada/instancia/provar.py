#!/usr/bin/env python3
"""Pedido 635: a trava de instancia, provada com os BINARIOS -- o `phxsqld` e a
CLI `phxsql`, dois processos de verdade na mesma pasta.

O teste de integracao (`crates/phxsql-store/tests/trava-de-instancia.rs`)
prova o motor; este prova o caso que o pedido nomeia como «o facil de
acontecer»: a CLI numa pasta que o `phxsqld` esta servindo. Cinco passos:

1. o servidor sobe, cria a tabela e grava uma linha pelo protocolo, e fica
   OCIOSO (nenhum pedido em voo);
2. a CLI tenta GRAVAR (`importar`) -> tem de ouvir `INSTANCIA_OCUPADA` com o
   pid do servidor, e a tabela nao pode ganhar linha;
3. a CLI LE (`listar`) -> tem de funcionar;
4. `kill -9` no servidor;
5. a CLI grava de novo -> tem de funcionar: a queda nao deixou trava eterna.

    python3 bancada/instancia/provar.py            # binarios de target/release
    PHX_BIN=target/debug python3 bancada/instancia/provar.py

Sai 0 so se os cinco passarem.
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
BIN = os.path.join(RAIZ, os.environ.get("PHX_BIN", os.path.join("target", "release")))
PHXSQLD = os.path.join(BIN, "phxsqld")
PHXSQL = os.path.join(BIN, "phxsql")
TOKEN = "token-de-servico"
DB = "loja"


def porta_livre():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


class Ligacao:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta))
        self.s.settimeout(20)
        self.f = self.s.makefile("rwb")
        self.ok({"op": "login", "usuario": "adm", "senha": "senha-do-adm"})

    def ok(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p.get("op"), r.get("erro")))
        return r.get("resultado")

    def fechar(self):
        for c in (self.f, self.s):
            try:
                c.close()
            except OSError:
                pass


def cli(*args):
    r = subprocess.run([PHXSQL, *args], capture_output=True, text=True, timeout=60)
    return r.returncode, r.stdout + r.stderr


def main():
    for b in (PHXSQLD, PHXSQL):
        if not os.path.exists(b):
            raise SystemExit("falta o binario %s -- compile antes" % b)
    raiz = tempfile.mkdtemp(prefix="phx-635-")
    porta = porta_livre()
    cfg = {
        "base": "base", "bind": "127.0.0.1:%d" % porta,
        "cifra_fio": {"exigir": False}, "token": TOKEN, "web": {"ligado": False},
        "usuarios": [{"login": "adm", "nome": "Adm", "id": 10, "nivel": "admin",
                      "senha_hash": hash_da_senha("senha-do-adm"),
                      "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                                      "excluir": True, "criar": True,
                                      "administrar": True}}}],
    }
    with open(os.path.join(raiz, "config.json"), "w") as f:
        json.dump(cfg, f)
    log = open(os.path.join(raiz, "servidor.log"), "w")
    srv = subprocess.Popen([PHXSQLD], cwd=raiz, stdout=log, stderr=subprocess.STDOUT,
                           stdin=subprocess.DEVNULL)
    passos = []
    try:
        for _ in range(200):
            try:
                socket.create_connection(("127.0.0.1", porta), 0.2).close()
                break
            except OSError:
                if srv.poll() is not None:
                    raise SystemExit("o servidor morreu ao subir")
                time.sleep(0.05)
        c = Ligacao(porta)
        c.ok({"op": "criar_database", "database": DB})
        c.ok({"op": "criar_tabela", "database": DB, "tabela": "clientes",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(20)"}],
              "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                           "primario": True}]})
        c.ok({"op": "inserir", "database": DB, "tabela": "clientes",
              "valores": {"id": 1, "nome": "servidor"}})
        c.fechar()
        pasta = os.path.join(raiz, "base", DB)
        csv = os.path.join(raiz, "linha.csv")
        with open(csv, "w") as f:
            f.write("id,nome\n2,cli\n")

        # 2. a CLI grava com o servidor de pe e ocioso
        rc, saida = cli("importar", pasta, "clientes", csv)
        recusou = rc != 0 and "ocupada por outro processo" in saida and ("processo %d" % srv.pid) in saida
        passos.append(("CLI grava com o phxsqld ocioso -> recusa 4008 com o pid", recusou, saida.strip()[-300:]))

        # 3. a CLI le
        rc, saida = cli("listar", pasta, "clientes")
        leu = rc == 0 and "servidor" in saida and "cli" not in saida
        passos.append(("CLI le com o phxsqld de pe (e a linha recusada nao entrou)", leu, saida.strip()[-300:]))

        # 4. kill -9 no servidor
        os.kill(srv.pid, signal.SIGKILL)
        srv.wait(10)
        passos.append(("kill -9 no phxsqld", srv.returncode == -signal.SIGKILL, str(srv.returncode)))

        # 5. a CLI grava de novo
        rc, saida = cli("importar", pasta, "clientes", csv)
        rc2, lista = cli("listar", pasta, "clientes")
        gravou = rc == 0 and rc2 == 0 and "cli" in lista
        passos.append(("CLI grava depois do kill -9 -> sem trava eterna", gravou, (saida + lista).strip()[-300:]))
    finally:
        if srv.poll() is None:
            srv.kill()
            srv.wait(10)
        shutil.rmtree(raiz, ignore_errors=True)

    falhou = 0
    for nome, ok, detalhe in passos:
        print("%s  %s" % ("PASSOU" if ok else "FALHOU", nome))
        if not ok:
            falhou += 1
            print("        " + detalhe.replace("\n", "\n        "))
    print("%d de %d" % (len(passos) - falhou, len(passos)))
    return 1 if falhou or len(passos) < 4 else 0


if __name__ == "__main__":
    sys.exit(main())
