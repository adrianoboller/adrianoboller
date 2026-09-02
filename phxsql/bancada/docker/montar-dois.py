#!/usr/bin/env python3
"""Dois PhxSql em conteiner: bases diferentes, tabelas diferentes, e a
comunicacao direta entre eles.

    python3 montar-dois.py

  phx-a  source  ->  base `loja`  com clientes e pedidos
  phx-b  replica ->  base `rh`    com funcionarios e cargos, E recebendo a
                     `loja` de A pelo canal direto

O que este roteiro prova, e por que os dois lados importam: se B so tivesse a
`loja`, seria replicacao e nao dois bancos; se B so tivesse o `rh`, seria dois
servidores isolados e nao comunicacao. As duas coisas juntas e o pedido.

A senha vira HASH pelo proprio binario, e o hash e o mesmo dos dois lados --
e dele que sai a chave do desafio-resposta que a replica usa para entrar no
source. Senha em claro nao entra em config nenhum.
"""
import json, os, shutil, subprocess, sys, time
from pathlib import Path

RAIZ = Path("/home/user/adrianoboller/phxsql")
AQUI = Path(__file__).resolve().parent
TRAB = AQUI / "docker"
IMAGEM = "phxsql:0.18.0"
REDE = "phxnet"
SENHA = "segredo-do-teste"
TOKEN = "token-do-teste-dois-docker"

def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, **k)

def hash_da_senha():
    r = subprocess.run([str(RAIZ / "target/release/phxsqld"), "--senha"],
                       input=SENHA, capture_output=True, text=True)
    for l in r.stdout.splitlines():
        if '"senha_hash"' in l:
            return l.split('"')[3]
    sys.exit(f"nao consegui o hash: {r.stdout}{r.stderr}")

def config(papel, hash_, origem=None):
    j = {
        "bind": "0.0.0.0:5000", "base": "/dados/base", "token": TOKEN,
        "max_linhas": 1000, "timeout_s": 30, "conexoes_max": 64,
        "web": {"ligado": True, "bind": "0.0.0.0:5001", "sessao_minutos": 60},
        "usuarios": [{"id": 1, "nome": "Adriano Boller", "login": "adm",
                      "senha_hash": hash_, "supervisor": True, "ativo": True,
                      "bases": {}}],
        "replicacao": {"papel": papel, "id_servidor": papel,
                       "imagem_da_linha": True},
    }
    if origem:
        j["replicacao"]["origens"] = [{
            "nome": "phx-a", "host": origem, "porta": 5000, "token": TOKEN,
            "usuario": "adm", "senha_hash": hash_, "databases": [],
        }]
    return j

def derrubar():
    for n in ("phx-a", "phx-b"):
        sh("docker", "rm", "-f", n)

def principal():
    derrubar()
    shutil.rmtree(TRAB, ignore_errors=True)
    h = hash_da_senha()
    print(f"· hash gerado pelo binario: {h[:26]}… ({len(h)} caracteres)")

    for nome, papel, origem, portas in (
        ("phx-a", "source",  None,    ("6500", "6501")),
        ("phx-b", "replica", "phx-a", ("6510", "6511")),
    ):
        d = TRAB / nome
        (d / "base").mkdir(parents=True)
        (d / "config.json").write_text(
            json.dumps(config(papel, h, origem), indent=2, ensure_ascii=False))
        os.chmod(d, 0o777)

    if sh("docker", "network", "inspect", REDE).returncode != 0:
        sh("docker", "network", "create", REDE)
    print(f"· rede {REDE} pronta")

    for nome, portas in (("phx-a", ("6500", "6501")), ("phx-b", ("6510", "6511"))):
        r = sh("docker", "run", "-d", "--name", nome, "--network", REDE,
               "--hostname", nome,
               "-p", f"{portas[0]}:5000", "-p", f"{portas[1]}:5001",
               "-v", f"{TRAB / nome}:/dados", IMAGEM)
        if r.returncode != 0:
            sys.exit(f"{nome} nao subiu: {r.stderr}")
        print(f"· {nome} de pe — dados {portas[0]}, web {portas[1]}")

    time.sleep(4)
    for nome in ("phx-a", "phx-b"):
        r = sh("docker", "logs", nome)
        print(f"\n-- o que {nome} imprimiu --")
        for l in (r.stdout + r.stderr).splitlines()[:4]:
            print("   ", l)
    return 0

if __name__ == "__main__":
    sys.exit(principal())
