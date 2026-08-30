#!/usr/bin/env python3
"""Sobe o servidor da prova de ODBC, monta os dados e roda a prova de ABI.

    cargo build --release
    python3 bancada/odbc/provar.py

# Por que este arquivo existe

A prova de ABI (`prova-abi.py`) e a montagem dos dados (`montar-dados.py`) ja
existiam e continuam donas do que provam. O que faltava era o passo do meio, e
ele estava escrito so em prosa no `docs/ODBC.md`: «um phxsqld SEU (a prova usou
127.0.0.1:5305, token prova-odbc, root/prova123)». Passo em prosa nao entra em
bateria -- e por isso a parte `odbc` da bateria unica era um PULO permanente,
com o motivo «precisa de um phxsqld montado».

Este script e o passo do meio, e mais nada: monta o servidor, chama as duas
provas que ja existiam, e mata pelo PID o processo que ele mesmo criou.

  --porta N   a porta de dados (padrao 6954, dentro da faixa desta frente)

Nunca usa `pkill`: ha outros phxsqld nesta maquina que nao sao nossos.
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
SO = os.path.join(RAIZ, "target", "release", "libphxsql_odbc.so")
BASE = "/tmp/phx-odbc"

TOKEN = "prova-odbc"
USUARIO, SENHA = "root", "prova123"


def arg(nome, padrao):
    return sys.argv[sys.argv.index(nome) + 1] if nome in sys.argv else padrao


PORTA = int(arg("--porta", "6954"))


def hash_da_senha(senha):
    """A linha pronta do config, gerada pelo proprio phxsqld.

    O sal muda a cada rodada, entao colar um hash fixo aqui seria colar um sal
    fixo -- e a senha em claro nunca chega ao arquivo.
    """
    r = subprocess.run([PHXSQLD, "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


def no_ar():
    try:
        socket.create_connection(("127.0.0.1", PORTA), timeout=0.5).close()
        return True
    except OSError:
        return False


def main():
    for caminho, como in ((PHXSQLD, "cargo build --release"),
                          (SO, "cargo build --release -p phxsql-odbc")):
        if not os.path.exists(caminho):
            sys.exit("nao achei %s -- rode `%s` antes" % (caminho, como))

    shutil.rmtree(BASE, ignore_errors=True)
    os.makedirs(BASE, exist_ok=True)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump({
            "base": "base",
            "bind": "127.0.0.1:%d" % PORTA,
            "token": TOKEN,
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": USUARIO,
                     "senha_hash": hash_da_senha(SENHA)},
        }, f, indent=2)

    log = open(os.path.join(BASE, "servidor.log"), "a")
    proc = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=log,
                            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    try:
        for _ in range(60):
            time.sleep(0.25)
            if no_ar():
                break
        else:
            sys.exit("o servidor nao subiu; veja %s/servidor.log" % BASE)
        print("· phxsqld pid %d na porta %d" % (proc.pid, PORTA))

        comuns = ["127.0.0.1", str(PORTA), TOKEN, USUARIO, SENHA]
        for passo, cmd in (
            ("montar os dados conhecidos",
             [sys.executable, os.path.join(AQUI, "montar-dados.py")] + comuns),
            ("a prova de ABI",
             [sys.executable, os.path.join(AQUI, "prova-abi.py"), SO] + comuns),
        ):
            print("\n=== %s ===" % passo)
            r = subprocess.run(cmd, cwd=AQUI)
            if r.returncode != 0:
                return r.returncode
        return 0
    finally:
        # Pelo PID, e so ele.
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=10)
        log.close()


if __name__ == "__main__":
    raise SystemExit(main())
