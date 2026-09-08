#!/usr/bin/env python3
"""A CIFRA DO FIO pelo driver ODBC, de ponta a ponta contra um phxsqld que
EXIGE o tunel (`cifra_fio.exigir: true`).

    cargo build --release
    cargo build --release -p phxsql-odbc
    python3 bancada/odbc/prova-cifra.py

# Por que este arquivo existe, ao lado do prova-abi.py

O `prova-abi.py` prova a ABI do driver contra um servidor em CLARO. O que
faltava era a outra ponta do gap da secao 10 do `docs/CIFRA-DO-FIO.md`: o
driver falando o APERTO de mao. Um servidor com `exigir: true` recusa todo
pedido fora do tunel -- entao um driver que ignore a opcao de cifra PARA nele,
e e exatamente isso que esta prova mede, nos dois sentidos.

# A prova real e nos DOIS sentidos

* **com a cifra** (CIFRA=1;CHAVE_DO_FIO=<pino>): a conexao fecha o aperto,
  loga POR DENTRO do tunel e o SELECT responde -- contra um servidor que
  recusa claro;
* **defeito reposto** (a mesma receita SEM a cifra): a conexao e recusada com
  erro nomeado. E o driver «velho», que fala claro, esbarrando no `exigir`.
* **pino errado**: a cifra liga, mas a chave apresentada nao e a pinada, e o
  aperto cai no cliente -- a defesa contra quem esta no meio.

Os dados sao montados POR DENTRO do tunel, com o cliente Noise independente da
`bancada/cifra-do-fio/prova.py` (Python puro, sem `makefile` a segurar o
descritor) -- porque com `exigir: true` nem a montagem pode falar claro.

Sobe um phxsqld PROPRIO na porta 6955 (dados), com um root e a estatica do fio
num arquivo, e mata SO o processo que criou, pelo PID. Nunca `pkill`: ha outros
phxsqld nesta maquina que nao sao nossos.
"""
import ctypes
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
BASE = "/tmp/phx-odbc-cifra"
ARQUIVO_DO_FIO = os.path.join(BASE, "chave-do-fio.hex")

TOKEN = "o token que o tunel esconde"
USUARIO, SENHA = "root", "prova123"


def arg(nome, padrao):
    return sys.argv[sys.argv.index(nome) + 1] if nome in sys.argv else padrao


PORTA = int(arg("--porta", "6955"))

# Reusa o cliente Noise independente da bancada da cifra, com as globais dele
# apontadas para o NOSSO servidor -- e a mesma implementacao ja provada, sem
# uma segunda copia da cripto aqui.
sys.path.insert(0, os.path.join(RAIZ, "bancada", "cifra-do-fio"))
import importlib

_cli = importlib.import_module("prova")
_cli.PORTA = PORTA
_cli.TOKEN = TOKEN

falhas = []


def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto!r}" +
          ("" if ok else f" (esperava {esperado!r})"))
    if not ok:
        falhas.append(rotulo)


def hash_da_senha(senha):
    r = subprocess.run([PHXSQLD, "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


def no_ar():
    try:
        socket.create_connection(("127.0.0.1", PORTA), timeout=0.5).close()
        return True
    except OSError:
        return False


# --------------------------------------------------------------------------
# O driver, pela ABI C -- o subconjunto que esta prova exercita.
# --------------------------------------------------------------------------
SUCESSO, COM_INFO, ERRO, INVALIDO, SEM_DADO = 0, 1, -1, -2, 100
ENV, DBC, STMT = 1, 2, 3
NTS = -3
C_CHAR = 1


def carregar_driver():
    d = ctypes.CDLL(SO, mode=ctypes.RTLD_LOCAL)
    # SQLRETURN e 16 bits: sem restype o ctypes le 32 e ve lixo legitimo da
    # ABI nos 16 de cima -- a mesma armadilha que o prova-abi.py documenta.
    for nome in ["SQLAllocHandle", "SQLFreeHandle", "SQLSetEnvAttr",
                 "SQLDriverConnect", "SQLDisconnect", "SQLExecDirect",
                 "SQLFetch", "SQLGetData", "SQLGetDiagRec"]:
        getattr(d, nome).restype = ctypes.c_short
    d.SQLGetData.argtypes = [ctypes.c_void_p, ctypes.c_ushort, ctypes.c_short,
                             ctypes.c_void_p, ctypes.c_ssize_t, ctypes.c_void_p]
    return d


def conectar(d, receita):
    """Devolve (codigo, dbc, estado, mensagem). dbc so serve se abriu."""
    env = ctypes.c_void_p()
    d.SQLAllocHandle(ENV, None, ctypes.byref(env))
    d.SQLSetEnvAttr(env, 200, ctypes.c_void_p(3), 0)  # ODBC 3
    dbc = ctypes.c_void_p()
    d.SQLAllocHandle(DBC, env, ctypes.byref(dbc))
    volta = ctypes.create_string_buffer(512)
    tam = ctypes.c_short(0)
    r = d.SQLDriverConnect(dbc, None, receita.encode(), NTS,
                           volta, 512, ctypes.byref(tam), 0)
    estado, msg = diag(d, DBC, dbc)
    return r, dbc, estado, msg


def diag(d, tipo, punho):
    estado = ctypes.create_string_buffer(6)
    nativo = ctypes.c_int(0)
    msg = ctypes.create_string_buffer(1024)
    tam = ctypes.c_short(0)
    d.SQLGetDiagRec(ctypes.c_short(tipo), punho, ctypes.c_short(1),
                    estado, ctypes.byref(nativo), msg, 1024, ctypes.byref(tam))
    return estado.value.decode(), msg.value.decode()


def contar_pelo_driver(d, dbc):
    stmt = ctypes.c_void_p()
    d.SQLAllocHandle(STMT, dbc, ctypes.byref(stmt))
    r = d.SQLExecDirect(stmt, b"SELECT COUNT(*) FROM clientes", NTS)
    if r not in (SUCESSO, COM_INFO):
        return None
    if d.SQLFetch(stmt) not in (SUCESSO, COM_INFO):
        return None
    buf = ctypes.create_string_buffer(64)
    ind = ctypes.c_ssize_t(0)
    d.SQLGetData(stmt, 1, C_CHAR, buf, 64, ctypes.byref(ind))
    return buf.value.decode()


# --------------------------------------------------------------------------
def montar_pelo_tunel(pino):
    """Login + tabela + linhas conhecidas, tudo por dentro do tunel."""
    c = _cli.Cliente()
    c.cifrar(pino)
    c.ok({"op": "login", "usuario": USUARIO, "senha": SENHA})
    c.ok({"op": "criar_database", "database": "loja"})
    r = c.fala({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
                "colunas": [
                    {"nome": "id", "tipo": "Int4", "obrigatoria": True},
                    {"nome": "nome", "tipo": "Str(40)"},
                    {"nome": "limite", "tipo": "Decimal(12,2)"},
                    {"nome": "desde", "tipo": "Date"},
                ],
                "indices": [{"nome": "porId", "colunas": ["id"],
                             "unico": True, "primario": True}]})
    for linha in [
        {"id": 1, "nome": "Adriano Boller", "limite": "15000.00",
         "desde": "2019-03-12"},
        {"id": 2, "nome": "Maria Operadora", "limite": "4200.50",
         "desde": "2021-07-01"},
        {"id": 3, "nome": "Carlos Consulta", "limite": None,
         "desde": "2024-12-25"},
    ]:
        c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "valores": linha})
    c.despedir()
    c.fechar()


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
            # exigir: true e o coracao da prova -- o servidor recusa claro.
            "cifra_fio": {"ligada": True, "exigir": True,
                          "arquivo": ARQUIVO_DO_FIO},
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": USUARIO,
                     "senha_hash": hash_da_senha(SENHA)},
        }, f, indent=2)

    # O pino: a chave publica que o servidor VAI apresentar. Sai do proprio
    # phxsqld, criando a estatica no arquivo -- e a mesma que ele apresenta,
    # porque le do mesmo lugar (docs/CIFRA-DO-FIO.md secao 6).
    saida = subprocess.run([PHXSQLD, "--chave-do-fio"], cwd=BASE,
                           capture_output=True, check=True)
    pino_hex = saida.stdout.decode().strip().splitlines()[-1].strip()
    pino = bytes.fromhex(pino_hex)
    print("· pino do servidor: %s…" % pino_hex[:16])

    log = open(os.path.join(BASE, "servidor.log"), "a")
    proc = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=log,
                            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    try:
        for _ in range(80):
            time.sleep(0.25)
            if no_ar():
                break
        else:
            sys.exit("o servidor nao subiu; veja %s/servidor.log" % BASE)
        print("· phxsqld pid %d na porta %d (exigir: true)" % (proc.pid, PORTA))

        print("\n=== montar os dados POR DENTRO do tunel ===")
        montar_pelo_tunel(pino)
        print("  ok   tres linhas gravadas pelo tunel")

        d = carregar_driver()
        cifrada = (f"Driver=PhxSql;Server=127.0.0.1;Port={PORTA};Token={TOKEN};"
                   f"UID={USUARIO};PWD={SENHA};Database=loja;"
                   f"CIFRA=1;CHAVE_DO_FIO={pino_hex}")
        clara = (f"Driver=PhxSql;Server=127.0.0.1;Port={PORTA};Token={TOKEN};"
                 f"UID={USUARIO};PWD={SENHA};Database=loja")
        pino_errado = "aa" * 32
        torta = (f"Driver=PhxSql;Server=127.0.0.1;Port={PORTA};Token={TOKEN};"
                 f"UID={USUARIO};PWD={SENHA};Database=loja;"
                 f"CIFRA=1;CHAVE_DO_FIO={pino_errado}")

        print("\n=== 1. com a cifra, o driver fecha o aperto e trabalha ===")
        r, dbc, estado, msg = conectar(d, cifrada)
        confere("SQLDriverConnect com CIFRA=1", r, SUCESSO)
        if r == SUCESSO:
            confere("SELECT COUNT(*) pelo tunel", contar_pelo_driver(d, dbc), "3")
            d.SQLDisconnect(dbc)

        print("\n=== 2. DEFEITO REPOSTO: sem a cifra, o servidor recusa ===")
        r, dbc, estado, msg = conectar(d, clara)
        confere("SQLDriverConnect em claro cai", r, ERRO)
        # O «porque» tem de estar no diagnostico: o driver velho recebe algo que
        # SABE exibir, e nao um silencio. A recusa chega pela via do login (a
        # primeira coisa que um cliente com UID manda), entao o SQLSTATE e o do
        # login; o que carrega o motivo e a mensagem.
        confere("o diagnostico nomeia a cifra exigida", "cifra do fio" in msg, True)
        print("       [%s] %s" % (estado, msg.strip()))
        confere("o diagnostico nao vaza a senha", SENHA in msg, False)

        print("\n=== 3. pino errado derruba o aperto ===")
        r, dbc, estado, msg = conectar(d, torta)
        confere("SQLDriverConnect com pino errado cai", r, ERRO)
        confere("o diagnostico nao carrega chave", pino_errado in msg, False)

        print("\n%s" % ("TODAS as conferencias passaram"
                        if not falhas else "FALHAS: " + ", ".join(falhas)))
        return 1 if falhas else 0
    finally:
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
