#!/usr/bin/env python3
"""Monta e sobe quatro servidores PhxSql: Master, Slave01, Slave02, Slave03.

    python3 bancada/replicacao/montar.py [diretorio]

Sobe tudo em 127.0.0.1, portas 5800 a 5803, e deixa os quatro no ar. A
topologia padrao e a que voce pediu -- tres espelhos do mesmo master:

    Master 5800 ──┬──► Slave01 5801
                  ├──► Slave02 5802
                  └──► Slave03 5803

Com `--cascata`, o Slave03 puxa do Slave01 em vez do master, para medir o
segundo salto:

    Master 5800 ──┬──► Slave01 5801 ──► Slave03 5803
                  └──► Slave02 5802

A SENHA NAO FICA EM CLARO em lugar nenhum: o `senha_hash` sai do proprio
`phxsqld --senha`, e e dele que a replica deriva a chave do desafio-resposta.
"""
import json
import os
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")

PORTA_MASTER = 5800
SLAVES = ["slave01", "slave02", "slave03"]
TOKEN = "espelho"
USUARIO = "adm"
SENHA = "segredo1"


def hash_da_senha(senha):
    """O hash sai do proprio servidor -- nao ha uma segunda implementacao."""
    saida = subprocess.run(
        [PHXSQLD, "--senha"], input=senha + "\n", capture_output=True, text=True
    ).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {
        "*": {
            "ler": True, "inserir": True, "alterar": True, "excluir": True,
            "criar": True, "administrar": True, "diario": True,
            "verificar": True, "replicar": True,
        }
    }


def config_master(h):
    return {
        "base": "base",
        "bind": f"127.0.0.1:{PORTA_MASTER}",
        "token": TOKEN,
        "web": {"ligado": False},
        # Sem isto o diario grava QUE a linha mudou e nao grava PARA QUE,
        # e as replicas nao tem o que aplicar. O arranque avisa.
        "replicacao": {
            "papel": "source",
            "imagem_da_linha": True,
            "id_servidor": "master",
        },
        "usuarios": [
            {"login": USUARIO, "nome": "Adriano", "id": 10, "senha_hash": h,
             "bases": permissoes()}
        ],
    }


def config_slave(h, n, porta_origem, nome_origem):
    return {
        "base": "base",
        "bind": f"127.0.0.1:{PORTA_MASTER + n}",
        "token": TOKEN,
        "web": {"ligado": False},
        # Uma replica escrita pela aplicacao quebra a numeracao dos rowids, e a
        # proxima inclusao vinda do source para a replicacao inteira.
        "somente_leitura": True,
        "replicacao": {
            "papel": "replica",
            "id_servidor": f"slave{n:02d}",
            # Ligada tambem na replica para ela poder ser origem de outra --
            # e o que faz a cascata funcionar.
            "imagem_da_linha": True,
            "origens": [
                {"nome": nome_origem, "host": "127.0.0.1", "porta": porta_origem,
                 "token": TOKEN, "usuario": USUARIO, "senha_hash": h,
                 "databases": ["loja"], "reconectar_em": 2}
            ],
        },
        "usuarios": [
            {"login": USUARIO, "nome": "Adriano", "id": 10, "senha_hash": h,
             "bases": permissoes()}
        ],
    }


def escrever(base, cascata):
    h = hash_da_senha(SENHA)
    os.makedirs(os.path.join(base, "master"), exist_ok=True)
    with open(os.path.join(base, "master", "config.json"), "w") as f:
        json.dump(config_master(h), f, indent=2)
    for i, nome in enumerate(SLAVES, start=1):
        origem = (PORTA_MASTER + 1, "slave01") if (cascata and i == 3) \
            else (PORTA_MASTER, "master")
        os.makedirs(os.path.join(base, nome), exist_ok=True)
        with open(os.path.join(base, nome, "config.json"), "w") as f:
            json.dump(config_slave(h, i, origem[0], origem[1]), f, indent=2)


def subir(base, nome):
    d = os.path.join(base, nome)
    log = open(os.path.join(d, "servidor.log"), "a")
    subprocess.Popen(["setsid", PHXSQLD], cwd=d, stdout=log,
                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)


def derrubar():
    subprocess.run(["pkill", "-x", "phxsqld"], check=False)
    time.sleep(1)


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    cascata = "--cascata" in sys.argv
    base = args[0] if args else "/tmp/phx-replicacao"

    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")

    derrubar()
    for d in ["master"] + SLAVES:
        caminho = os.path.join(base, d, "base")
        if os.path.exists(caminho):
            subprocess.run(["rm", "-rf", caminho], check=False)
    escrever(base, cascata)

    # O master sobe primeiro e sozinho: as replicas criam a tabela a partir do
    # esquema DELE, entao nao ha o que criar antes de ele existir.
    subir(base, "master")
    time.sleep(2)
    for nome in SLAVES:
        subir(base, nome)
    time.sleep(2)
    print(f"quatro servidores no ar em {base}")
    print(f"  master  127.0.0.1:{PORTA_MASTER}")
    for i, nome in enumerate(SLAVES, start=1):
        de = "slave01" if (cascata and i == 3) else "master"
        print(f"  {nome} 127.0.0.1:{PORTA_MASTER + i}  puxando de {de}")
