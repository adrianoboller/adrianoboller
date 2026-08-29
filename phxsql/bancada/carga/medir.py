#!/usr/bin/env python3
"""Mede a carga de linhas pela rede: uma a uma contra `inserir_lote`.

    cargo build --release
    python3 bancada/carga/medir.py [n_linhas]

O numero de 20.000 linhas em lote saiu de uma medicao feita a mao e ficou na
documentacao sem script -- ou seja, sem como refazer quando o motor mudasse. E
o motor mudou: o cache de paginas do `.ndx` entrou na 0.17.0. Este arquivo
existe para o numero nao envelhecer calado de novo.

As duas metades fazem **o mesmo trabalho**: as mesmas linhas, a mesma tabela, os
mesmos dois indices, o mesmo servidor. Muda so quantas viagens de rede, quantas
aberturas de tabela e quantos `fsync` a carga custa.

A ultima linha e `RESULTADO <json>`.
"""
import json
import os
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = os.environ.get("PHX_CARGA", "/tmp/phx-carga")
PORTA = 5810
TOKEN = "carga"
USUARIO = "adm"
SENHA = "segredo1"
POR_LOTE = 5_000


def hash_da_senha(senha):
    """O hash sai do proprio servidor -- nao ha uma segunda implementacao."""
    saida = subprocess.run(
        [PHXSQLD, "--senha"], input=senha + "\n", capture_output=True, text=True
    ).stdout
    return saida.split('": "')[1].split('"')[0]


def subir():
    subprocess.run(["pkill", "-x", "phxsqld"], check=False)
    time.sleep(1)
    subprocess.run(["rm", "-rf", BASE], check=False)
    os.makedirs(BASE, exist_ok=True)
    config = {
        "base": "base",
        "bind": f"127.0.0.1:{PORTA}",
        "token": TOKEN,
        "web": {"ligado": False},
        "usuarios": [
            {
                "login": USUARIO, "nome": "Adriano", "id": 10,
                "senha_hash": hash_da_senha(SENHA),
                "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                                "excluir": True, "criar": True,
                                "administrar": True, "verificar": True}},
            }
        ],
    }
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump(config, f, indent=2)
    log = open(os.path.join(BASE, "servidor.log"), "a")
    subprocess.Popen(["setsid", PHXSQLD], cwd=BASE, stdout=log,
                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    time.sleep(2)


def liga():
    s = socket.create_connection(("127.0.0.1", PORTA))
    f = s.makefile("rwb")

    def fala(p):
        p.setdefault("token", TOKEN)
        f.write((json.dumps(p) + "\n").encode())
        f.flush()
        r = json.loads(f.readline().decode())
        if not r.get("ok"):
            raise SystemExit(f"{p['op']}: {r.get('erro')}")
        return r["resultado"]

    fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
    return fala


def criar(fala, tabela):
    fala({"op": "criar_tabela", "database": "loja", "tabela": tabela,
          "colunas": [
              {"nome": "id", "tipo": "Int8", "obrigatoria": True},
              {"nome": "produto", "tipo": "Str(40)", "obrigatoria": True},
              {"nome": "cidade", "tipo": "Str(20)"},
              {"nome": "valor", "tipo": "Int8"}],
          "indices": [
              {"nome": "porId", "colunas": ["id"], "unico": True,
               "primario": True},
              {"nome": "porCidade", "colunas": ["cidade"]}]})


CIDADES = ["Blumenau", "Joinville", "Itajai", "Curitiba",
           "Chapeco", "Lages", "Florianopolis", "Criciuma"]


def linhas(n):
    return [{"id": i, "produto": f"Produto {i:08d}",
             "cidade": CIDADES[i % len(CIDADES)], "valor": i * 7}
            for i in range(1, n + 1)]


def uma_a_uma(fala, n):
    criar(fala, "uma_a_uma")
    ls = linhas(n)
    t = time.perf_counter()
    for l in ls:
        fala({"op": "inserir", "database": "loja", "tabela": "uma_a_uma",
              "linha": l})
    return time.perf_counter() - t


def em_lote(fala, n):
    criar(fala, "em_lote")
    ls = linhas(n)
    t = time.perf_counter()
    for i in range(0, n, POR_LOTE):
        fala({"op": "inserir_lote", "database": "loja", "tabela": "em_lote",
              "linhas": ls[i:i + POR_LOTE]})
    return time.perf_counter() - t


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 20_000
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")

    subir()
    fala = liga()
    fala({"op": "criar_database", "database": "loja"})

    print(f"=== carga de {n} linhas pela rede, dois indices ===\n")
    s_uma = uma_a_uma(fala, n)
    print(f"  uma a uma      {s_uma:7.2f}s  {n / s_uma:9.0f} linhas/s")
    s_lote = em_lote(fala, n)
    print(f"  lotes de {POR_LOTE:<5} {s_lote:7.2f}s  {n / s_lote:9.0f} linhas/s")
    print(f"\n  o lote e {s_uma / s_lote:.1f}x mais rapido")

    # Conferencia: as duas metades tem de ter gravado o mesmo tanto. Comparar
    # tempo de trabalhos diferentes seria a armadilha que a bancada ja pegou
    # duas vezes.
    for tab in ("uma_a_uma", "em_lote"):
        r = fala({"op": "verificar", "database": "loja", "tabela": tab})
        assert r.get("registros") == n, f"{tab}: {r.get('registros')} de {n}"

    print("\nRESULTADO " + json.dumps({
        "linhas": n, "por_lote": POR_LOTE,
        "uma_a_uma_s": round(s_uma, 3),
        "uma_a_uma_por_s": round(n / s_uma),
        "em_lote_s": round(s_lote, 3),
        "em_lote_por_s": round(n / s_lote),
        "ganho": round(s_uma / s_lote, 2),
    }))
    subprocess.run(["pkill", "-x", "phxsqld"], check=False)
