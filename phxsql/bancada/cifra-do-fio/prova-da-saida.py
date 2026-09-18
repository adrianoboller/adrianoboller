#!/usr/bin/env python3
"""A virada da SAIDA, provada pelo SOQUETE -- um master e uma replica de verdade.

Os dois sentidos, e e por isso que a prova vale:

  (A) PADRAO NOVO: a replica NAO escreve `cifra` na origem. Ela pede o aperto,
      o master (que exige de fabrica) deixa entrar, e a linha atravessa.
  (B) DEFEITO REPOSTO: a mesma replica com `"cifra": false` escrito -- que e o
      PADRAO VELHO. O master recusa, e nada atravessa.

O cliente que escreve no master e o `phxsqlcmd`, que fala o aperto desde o
pedido 370 -- teste unitario nao prova queda de conexao nem aperto de mao.
"""
import json, os, shutil, socket, subprocess, time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PHXSQLD = f"{RAIZ}/target/debug/phxsqld"
CMD = f"{RAIZ}/target/debug/phxsqlcmd"
BASE = "/tmp/phx-prova-da-saida"
TOKEN = "token-da-prova"
USUARIO, SENHA = "adm", "segredo1"
# Faixa propria, para nunca esbarrar na 7210 da `prova.py` ao lado nem nas das
# outras bancadas.
P_MASTER, P_REPLICA = 7220, 7221
PROCS = []


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True, "excluir": True,
                  "criar": True, "administrar": True, "diario": True,
                  "verificar": True, "replicar": True}}


def subir(nome, cfg):
    d = os.path.join(BASE, nome)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log, stderr=subprocess.STDOUT,
                         stdin=subprocess.DEVNULL)
    PROCS.append(p)
    return p


def derrubar():
    for p in PROCS:
        if p.poll() is None:
            p.terminate()
    for p in PROCS:
        try:
            p.wait(timeout=15)
        except subprocess.TimeoutExpired:
            p.kill()
    PROCS.clear()


def esperar(porta, s=20):
    fim = time.monotonic() + s
    while time.monotonic() < fim:
        try:
            socket.create_connection(("127.0.0.1", porta), timeout=1).close()
            return True
        except OSError:
            time.sleep(0.2)
    return False


def console(porta, linha):
    """Um comando pelo console -- que nasce CIFRADO."""
    r = subprocess.run([CMD, "--porta", str(porta), "--token", TOKEN,
                        "--usuario", USUARIO, "--comando", linha],
                       capture_output=True, text=True,
                       env={**os.environ, "PHXSQL_SENHA": SENHA}, timeout=60)
    return r.returncode, (r.stdout + r.stderr).strip()


def rodada(escreve_o_escape):
    shutil.rmtree(BASE, ignore_errors=True)
    h = hash_da_senha(SENHA)
    master = {"base": "base", "bind": f"127.0.0.1:{P_MASTER}", "token": TOKEN,
              "web": {"ligado": False},
              "replicacao": {"papel": "source", "id_servidor": "master",
                             "imagem_da_linha": True},
              "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                            "senha_hash": h, "bases": permissoes()}]}
    origem = {"nome": "master", "host": "127.0.0.1", "porta": P_MASTER,
              "token": TOKEN, "usuario": USUARIO, "senha_hash": h,
              "reconectar_em": 1}
    if escreve_o_escape:
        origem["cifra"] = False        # o PADRAO VELHO, reposto por escrito
    replica = {"base": "base", "bind": f"127.0.0.1:{P_REPLICA}", "token": TOKEN,
               "web": {"ligado": False}, "somente_leitura": True,
               "replicacao": {"papel": "replica", "id_servidor": "replica01",
                              "imagem_da_linha": True, "origens": [origem]},
               "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                             "senha_hash": h, "bases": permissoes()}]}
    subir("master", master)
    assert esperar(P_MASTER), "o master nao subiu"
    # A tabela e a linha ANTES da replica subir nao provariam o fio: a replica
    # puxaria o passado. Sobe a replica primeiro e escreve depois.
    codigo, saida = console(P_MASTER, json.dumps(
        {"op": "criar_database", "database": "loja"}))
    print(f"   criar_database: {codigo} {saida[:120]}")
    codigo, saida = console(P_MASTER, json.dumps(
        {"op": "criar_tabela", "database": "loja", "tabela": "clientes",
         "motivo_obrigatorio": False,
         "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                     {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True}],
         "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                      "primario": True}]}))
    print(f"   criar_tabela:   {codigo} {saida[:120]}")
    subir("replica", replica)
    assert esperar(P_REPLICA), "a replica nao subiu"
    time.sleep(3)
    codigo, saida = console(P_MASTER, json.dumps(
        {"op": "inserir", "database": "loja", "tabela": "clientes",
         "linha": {"id": 1, "nome": "Ana"}}))
    print(f"   inserir:        {codigo} {saida[:120]}")
    time.sleep(6)
    codigo, saida = console(P_REPLICA, json.dumps(
        {"op": "varrer", "database": "loja", "tabela": "clientes", "max": 10}))
    chegou = '"nome":"Ana"' in saida.replace(" ", "") or "Ana" in saida
    log = open(os.path.join(BASE, "master", "acessos.log")).read() \
        if os.path.exists(os.path.join(BASE, "master", "acessos.log")) else ""
    cifrar = log.count('"op":"cifrar"')
    recusas = log.count("cifra do fio") + log.count("aperto de mao")
    diario = open(os.path.join(BASE, "replica", "servidor.log")).read()
    derrubar()
    return chegou, cifrar, recusas, saida, diario


print("=" * 72)
print("(A) PADRAO NOVO -- a replica NAO escreve `cifra`")
print("=" * 72)
chegou, cifrar, recusas, saida, diario = rodada(False)
print(f"   linha na replica: {chegou}")
print(f"   master acessos.log: op=cifrar {cifrar}x")
print(f"   varrer na replica: {saida[:200]}")
for l in diario.splitlines():
    if "cifra" in l.lower() or "aperto" in l.lower() or "origem" in l.lower():
        print("   replica.log:", l[:160])
a = (chegou, cifrar)

print()
print("=" * 72)
print('(B) DEFEITO REPOSTO -- `"cifra": false` escrito (o padrao VELHO)')
print("=" * 72)
chegou, cifrar, recusas, saida, diario = rodada(True)
print(f"   linha na replica: {chegou}")
print(f"   master acessos.log: op=cifrar {cifrar}x")
print(f"   varrer na replica: {saida[:200]}")
for l in diario.splitlines():
    if "cifra" in l.lower() or "aperto" in l.lower():
        print("   replica.log:", l[:200])
b = (chegou, cifrar)

print()
print("VEREDITO:", "OK" if (a[0] and a[1] > 0 and not b[0]) else "FALHOU")
