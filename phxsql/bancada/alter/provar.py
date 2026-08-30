#!/usr/bin/env python3
"""A prova REAL do `acrescentar_coluna`, pelo soquete e com replicacao.

    python3 bancada/alter/provar.py

Teste unitario nao prova o servidor: soquete prova. Este script sobe DOIS
phxsqld de verdade (source 7150, replica 7152), monta uma tabela com dado,
acrescenta uma coluna, e so entao pergunta se tudo continua funcionando:
ler, inserir, atualizar, marcar, restaurar, backup, restaurar backup e
replicar.

# O que so ele acha

Os quinze testes de `acrescentar-coluna.rs` provam o formato. Tres coisas
deste script nenhum deles alcanca:

1. a operacao existe **pelo protocolo**, com o portao de permissao certo;
2. o **backup** feito depois da alteracao volta com a coluna e com os rowids;
3. o que acontece com uma **replica que ainda nao alterou** -- e o que
   acontece quando ela alterar. A resposta esta nos passos 9 e 10, e nao e
   obvia: a replica PARA de aplicar em vez de aceitar um payload de outra
   largura, e volta a andar sozinha, do ponto em que parou, assim que os dois
   lados tem o mesmo esquema.

Mata so os PIDs que ele mesmo subiu. Nunca `pkill -f`.
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = sys.argv[1] if len(sys.argv) > 1 else os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = "/tmp/phx-alter-prova"

PORTA_SOURCE = 7150
PORTA_REPLICA = 7152
TOKEN = "alterprova"
USUARIO = "adm"
SENHA = "segredo1"

PIDS = []
FALHAS = []
PASSOS = []


def ok(nome, condicao, detalhe=""):
    PASSOS.append((nome, bool(condicao), detalhe))
    if not condicao:
        FALHAS.append(f"{nome}: {detalhe}")
    print(("  OK   " if condicao else "  FALHA") + f"  {nome}" + (f"  -- {detalhe}" if detalhe else ""))


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True, "excluir": True,
                  "criar": True, "administrar": True, "diario": True,
                  "verificar": True, "replicar": True}}


def escrever_config(h):
    os.makedirs(os.path.join(BASE, "source"), exist_ok=True)
    os.makedirs(os.path.join(BASE, "replica"), exist_ok=True)
    with open(os.path.join(BASE, "source", "config.json"), "w") as f:
        json.dump({
            "base": "base",
            "bind": f"127.0.0.1:{PORTA_SOURCE}",
            "token": TOKEN,
            "web": {"ligado": False},
            "espelho": True,
            "replicacao": {"papel": "source", "imagem_da_linha": True,
                           "id_servidor": "source"},
            "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                          "senha_hash": h, "bases": permissoes()}],
        }, f, indent=2)
    with open(os.path.join(BASE, "replica", "config.json"), "w") as f:
        json.dump({
            "base": "base",
            "bind": f"127.0.0.1:{PORTA_REPLICA}",
            "token": TOKEN,
            "web": {"ligado": False},
            "somente_leitura": True,
            "replicacao": {
                "papel": "replica", "id_servidor": "replica",
                "imagem_da_linha": True,
                "origens": [{"nome": "source", "host": "127.0.0.1",
                             "porta": PORTA_SOURCE, "token": TOKEN,
                             "usuario": USUARIO, "senha_hash": h,
                             "databases": ["loja"], "reconectar_em": 1}],
            },
            "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                          "senha_hash": h, "bases": permissoes()}],
        }, f, indent=2)


def subir(nome):
    d = os.path.join(BASE, nome)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log, stderr=subprocess.STDOUT,
                         stdin=subprocess.DEVNULL)
    PIDS.append(p.pid)
    return p


def esperar_porta(porta, prazo=20):
    fim = time.time() + prazo
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", porta), 0.5).close()
            return True
        except OSError:
            time.sleep(0.2)
    return False


def liga(porta):
    s = socket.create_connection(("127.0.0.1", porta))
    f = s.makefile("rwb")

    def fala(p):
        p.setdefault("token", TOKEN)
        f.write((json.dumps(p) + "\n").encode())
        f.flush()
        return json.loads(f.readline().decode())

    r = fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
    if not r.get("ok"):
        raise SystemExit(f"login na porta {porta}: {r}")
    return fala


def derrubar():
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    time.sleep(1)
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD}")
    shutil.rmtree(BASE, ignore_errors=True)
    escrever_config(hash_da_senha(SENHA))
    subir("source")
    subir("replica")
    if not esperar_porta(PORTA_SOURCE) or not esperar_porta(PORTA_REPLICA):
        derrubar()
        sys.exit("os servidores nao subiram")

    S = liga(PORTA_SOURCE)

    print("\n== 1. a tabela com dado de verdade ==")
    S({"op": "criar_database", "database": "loja"})
    r = S({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
           "colunas": [
               {"nome": "id", "tipo": "Int8", "obrigatoria": True},
               {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
               {"nome": "cidade", "tipo": "Str(30)"},
           ],
           "indices": [{"nome": "pk_id", "colunas": ["id"], "unico": True, "primario": True},
                       {"nome": "porNome", "colunas": ["nome"]}]})
    ok("criar_tabela", r.get("ok"), json.dumps(r)[:200])

    linhas = [[i, f"Cliente {i:04}", "Blumenau" if i % 2 else "Joinville"] for i in range(1, 51)]
    r = S({"op": "inserir_lote", "database": "loja", "tabela": "clientes", "linhas": linhas})
    ok("inserir_lote de 50", r.get("ok") and r["resultado"]["gravadas"] == 50, json.dumps(r)[:200])

    r = S({"op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 10,
           "motivo": "prova", "fisico": True})
    ok("excluir de vez antes de alterar", r.get("ok"), json.dumps(r)[:200])
    r = S({"op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 11, "motivo": "prova"})
    ok("marcar antes de alterar", r.get("ok"), json.dumps(r)[:200])

    antes = S({"op": "varrer", "database": "loja", "tabela": "clientes", "visao": "todas", "limite": 100})
    antes_linhas = antes["resultado"]["linhas"]
    print(f"     retrato antes: {len(antes_linhas)} linha(s)")

    print("\n== 2. esperar a replica convergir ANTES de alterar ==")
    R = liga(PORTA_REPLICA)
    conv = False
    for _ in range(60):
        rr = R({"op": "varrer", "database": "loja", "tabela": "clientes", "visao": "todas", "limite": 100})
        if rr.get("ok") and len(rr["resultado"]["linhas"]) == len(antes_linhas):
            conv = True
            break
        time.sleep(0.5)
    ok("a replica converge antes de alterar", conv)

    print("\n== 3. acrescentar a coluna no SOURCE ==")
    r = S({"op": "acrescentar_coluna", "database": "loja", "tabela": "clientes",
           "nome": "situacao", "tipo": "Str(12)", "caption": "Situação", "padrao": "ativo"})
    ok("acrescentar_coluna responde ok", r.get("ok"), json.dumps(r)[:300])
    if r.get("ok"):
        res = r["resultado"]
        print(f"     slots reescritos: {res['slots_reescritos']}  ms: {res['ms']:.2f}  "
              f"posicao: {res['posicao']}  indices_refeitos: {res['indices_refeitos']}")
        ok("a coluna entrou antes das de sistema", res["posicao"] == 3, str(res["posicao"]))
        ok("o indice nao foi refeito", res["indices_refeitos"] is False)

    print("\n== 4. o rowid e o conteudo de cada linha ==")
    depois = S({"op": "varrer", "database": "loja", "tabela": "clientes", "visao": "todas", "limite": 100})
    dl = depois["resultado"]["linhas"]
    ok("mesma quantidade de linhas", len(dl) == len(antes_linhas), f"{len(antes_linhas)} -> {len(dl)}")
    iguais = all(a["rowid"] == d["rowid"] and a["nome"] == d["nome"] and a["id"] == d["id"]
                 for a, d in zip(antes_linhas, dl))
    ok("cada rowid continua com a mesma linha", iguais)
    ok("a coluna nova veio com o padrao", all(d.get("situacao") == "ativo" for d in dl),
       json.dumps(dl[0])[:200])
    ok("o slot excluido continua excluido", all(d["rowid"] != 10 for d in dl))
    marcada = [d for d in dl if d["rowid"] == 11]
    ok("a linha marcada continua marcada", marcada and marcada[0].get("softdeleted") is True,
       json.dumps(marcada)[:200] if marcada else "sumiu")

    print("\n== 5. a busca pelo indice, que nao foi refeito ==")
    r = S({"op": "buscar", "database": "loja", "tabela": "clientes",
           "indice": "porNome", "chave": ["Cliente 0007"]})
    achou = r.get("ok") and r["resultado"]["linhas"] and r["resultado"]["linhas"][0]["rowid"] == 7
    ok("o indice acha a linha certa", achou, json.dumps(r)[:200])

    r = S({"op": "verificar", "database": "loja", "tabela": "clientes"})
    ok("verificar passa", r.get("ok"), json.dumps(r)[:300])

    print("\n== 6. a tabela continua viva: inserir, atualizar, marcar, restaurar ==")
    r = S({"op": "inserir", "database": "loja", "tabela": "clientes",
           "valores": {"id": 51, "nome": "Cliente 0051", "cidade": "Lages", "situacao": "novo"}})
    ok("inserir com a coluna nova", r.get("ok") and r["resultado"]["rowid"] == 51, json.dumps(r)[:200])

    atual = S({"op": "ler", "database": "loja", "tabela": "clientes", "rowid": 5})["resultado"]
    atual["situacao"] = "inativo"
    r = S({"op": "atualizar", "database": "loja", "tabela": "clientes", "rowid": 5,
           "valores": atual})
    ok("atualizar so a coluna nova", r.get("ok"), json.dumps(r)[:200])
    r = S({"op": "ler", "database": "loja", "tabela": "clientes", "rowid": 5})
    ok("a alteracao ficou", r["resultado"]["situacao"] == "inativo", json.dumps(r)[:200])

    r = S({"op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 6, "motivo": "prova"})
    ok("marcar depois de alterar", r.get("ok"), json.dumps(r)[:200])
    r = S({"op": "restaurar", "database": "loja", "tabela": "clientes", "rowid": 6, "motivo": "prova"})
    ok("restaurar depois de alterar", r.get("ok"), json.dumps(r)[:200])
    r = S({"op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 7,
           "motivo": "prova", "fisico": True})
    ok("excluir de vez depois de alterar", r.get("ok"), json.dumps(r)[:200])

    print("\n== 7. a guarda: coluna obrigatoria sem padrao numa tabela com linha ==")
    r = S({"op": "acrescentar_coluna", "database": "loja", "tabela": "clientes",
           "nome": "cnpj", "tipo": "Str(18)", "obrigatoria": True})
    ok("recusa a obrigatoria sem padrao", not r.get("ok") and "inventar" in json.dumps(r),
       json.dumps(r)[:300])
    r = S({"op": "acrescentar_coluna", "database": "loja", "tabela": "clientes",
           "nome": "nome", "tipo": "Str(10)"})
    ok("recusa nome repetido", not r.get("ok"), json.dumps(r)[:200])
    r = S({"op": "acrescentar_coluna", "database": "loja", "tabela": "clientes",
           "nome": "rownum", "tipo": "UInt8"})
    ok("recusa nome de coluna do motor", not r.get("ok"), json.dumps(r)[:200])

    print("\n== 8. backup e restauracao ==")
    destino = os.path.join(BASE, "backup")
    r = S({"op": "backup", "database": "loja", "destino": destino, "zip": True})
    ok("backup grava", r.get("ok"), json.dumps(r)[:300])
    origem = (r.get("resultado") or {}).get("arquivo") or (r.get("resultado") or {}).get("destino") or destino
    r = S({"op": "restaurar_backup", "origem": origem, "database": "loja_volta"})
    ok("restaurar_backup para outro nome", r.get("ok"), json.dumps(r)[:400])
    r = S({"op": "varrer", "database": "loja_volta", "tabela": "clientes", "visao": "todas", "limite": 100})
    volta = r.get("resultado", {}).get("linhas", [])
    ok("o backup restaurado tem a coluna nova",
       bool(volta) and all("situacao" in v for v in volta), json.dumps(volta[:1])[:300])
    ok("o backup restaurado preservou os rowids",
       [v["rowid"] for v in volta] == [d["rowid"] for d in
                                       S({"op": "varrer", "database": "loja", "tabela": "clientes",
                                          "visao": "todas", "limite": 100})["resultado"]["linhas"]])

    print("\n== 9. a replica: o que acontece com o esquema mudado de um lado so ==")
    # A replica NAO ganhou a coluna. O evento novo chega com o payload largo.
    S({"op": "inserir", "database": "loja", "tabela": "clientes",
       "valores": {"id": 52, "nome": "Cliente 0052", "cidade": "Itajai", "situacao": "novo"}})
    time.sleep(4)
    rr = R({"op": "varrer", "database": "loja", "tabela": "clientes", "visao": "todas", "limite": 100})
    tem52 = any(v["rowid"] == 52 for v in rr.get("resultado", {}).get("linhas", []))
    ok("a replica NAO aplica o evento de esquema novo (e para em vez de mentir)", not tem52,
       "a replica aceitou um payload de outra largura!" if tem52 else "")

    print("\n== 10. alterar a replica tambem, e a replicacao volta ==")
    # A porta de dados da replica e somente-leitura; o `acrescentar_coluna`
    # e escrita, entao a alteracao vai pelo arquivo: paramos, alteramos com o
    # mesmo comando num servidor sem somente_leitura, e subimos de volta.
    # Aqui usamos o proprio caminho do protocolo, com a replica religada.
    cfg = os.path.join(BASE, "replica", "config.json")
    with open(cfg) as f:
        c = json.load(f)
    c["somente_leitura"] = False
    with open(cfg, "w") as f:
        json.dump(c, f, indent=2)
    # Derruba so a replica e sobe de novo.
    pid = PIDS[1]
    os.kill(pid, signal.SIGTERM)
    time.sleep(1.5)
    p = subprocess.Popen([PHXSQLD], cwd=os.path.join(BASE, "replica"),
                         stdout=open(os.path.join(BASE, "replica", "servidor.log"), "a"),
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    PIDS[1] = p.pid
    esperar_porta(PORTA_REPLICA)
    R = liga(PORTA_REPLICA)
    r = R({"op": "acrescentar_coluna", "database": "loja", "tabela": "clientes",
           "nome": "situacao", "tipo": "Str(12)", "caption": "Situação", "padrao": "ativo"})
    ok("a replica aceita a mesma alteracao", r.get("ok"), json.dumps(r)[:300])

    convergiu = False
    for _ in range(60):
        rr = R({"op": "varrer", "database": "loja", "tabela": "clientes", "visao": "todas", "limite": 100})
        if any(v["rowid"] == 52 for v in rr.get("resultado", {}).get("linhas", [])):
            convergiu = True
            break
        time.sleep(0.5)
    ok("com o esquema igual dos dois lados a replicacao volta a andar", convergiu)

    if convergiu:
        alvo = [v for v in rr["resultado"]["linhas"] if v["rowid"] == 52][0]
        ok("a linha replicada trouxe a coluna nova", alvo.get("situacao") == "novo",
           json.dumps(alvo)[:200])
        so = S({"op": "varrer", "database": "loja", "tabela": "clientes", "visao": "todas", "limite": 100})["resultado"]["linhas"]
        re_ = rr["resultado"]["linhas"]
        ok("source e replica com o mesmo conjunto de rowids",
           [v["rowid"] for v in so] == [v["rowid"] for v in re_],
           f"{[v['rowid'] for v in so][:8]} vs {[v['rowid'] for v in re_][:8]}")

    print("\n== resumo ==")
    print(f"  {sum(1 for _, o, _ in PASSOS if o)}/{len(PASSOS)} passos")
    for f in FALHAS:
        print("  FALHA:", f)
    return 1 if FALHAS else 0


if __name__ == "__main__":
    codigo = 1
    try:
        codigo = main()
    finally:
        derrubar()
    sys.exit(codigo)
