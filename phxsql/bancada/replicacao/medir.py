#!/usr/bin/env python3
"""Mede a replicacao dos quatro servidores que o `montar.py` subiu.

    python3 bancada/replicacao/montar.py /tmp/phx-replicacao
    python3 bancada/replicacao/medir.py [n_linhas]

O que ela mede, e por que cada um:

1. **Atraso ate as tres replicas**, por tipo de escrita. E o numero que decide
   se uma replica serve para consulta -- e ele e dominado pelo intervalo do
   laco, nao pelo trabalho.
2. **Vazao de aplicacao**, com carga grande o bastante para o tempo de aplicar
   dominar a espera. E o numero que decide se a replica ACOMPANHA a escrita.
3. **Retomada depois de queda**: derruba o slave03, escreve, sobe de volta.
4. **Igualdade**, e nao contagem. Compara um SHA-256 de cada linha inteira, com
   rowid e numero de ordem -- contar linhas nao acha uma que atravessou errada.

A ultima linha e `RESULTADO <json>`, para nao ter de adivinhar nada.
"""
import hashlib
import json
import os
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = os.environ.get("PHX_REPLICACAO", "/tmp/phx-replicacao")

PORTAS = {"master": 5800, "slave01": 5801, "slave02": 5802, "slave03": 5803}
TOKEN = "espelho"


def liga(porta):
    s = socket.create_connection(("127.0.0.1", porta))
    f = s.makefile("rwb")

    def fala(p):
        p.setdefault("token", TOKEN)
        f.write((json.dumps(p) + "\n").encode())
        f.flush()
        return json.loads(f.readline().decode())

    r = fala({"op": "login", "usuario": "adm", "senha": "segredo1"})
    if not r.get("ok"):
        raise SystemExit(f"login na porta {porta}: {r}")
    return fala


C = {}


def posicao(fala):
    r = fala({"op": "posicao", "database": "loja"})["resultado"]
    return r["tabelas"]["clientes"]["eventos"]


def posicoes():
    return {n: posicao(f) for n, f in C.items()}


def retrato(fala):
    """Um SHA-256 da tabela inteira, lido pelo cursor -- pagina a pagina.

    Contar linhas nao prova nada: duas tabelas com o mesmo numero de linhas
    podem ter conteudo diferente. Isto compara cada campo de cada linha, com o
    rowid junto -- que e justamente o que a replicacao promete reproduzir sem
    transmitir.
    """
    h = hashlib.sha256()
    linhas, depois = 0, 0
    while True:
        d = fala({"op": "varrer", "database": "loja", "tabela": "clientes",
                  "max": 2000, "depois": depois, "visao": "todas"})["resultado"]
        for l in d["linhas"]:
            h.update(json.dumps(l, sort_keys=True, ensure_ascii=False).encode())
            linhas += 1
        if not d["ha_mais"] or not d["linhas"]:
            break
        depois = d["cursor_fim"]
    return linhas, h.hexdigest()[:16]


def esperar(alvo, segundos=600):
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < segundos:
        if all(v >= alvo for v in posicoes().values()):
            return time.perf_counter() - t0
        time.sleep(0.02)
    return None


def semear(m, n):
    m({"op": "criar_database", "database": "loja"})
    m({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                   {"nome": "cidade", "tipo": "Str(30)"},
                   {"nome": "limite", "tipo": "Decimal(12,2)"},
                   {"nome": "ficha", "tipo": "Memo"}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    cid = ["Blumenau", "Joinville", "Itajai", "Curitiba", "Florianopolis"]
    t0 = time.perf_counter()
    i = 0
    while i < n:
        linhas = [{"id": k, "nome": f"Cliente {k:07d}", "cidade": cid[k % 5],
                   "limite": f"{k}.50",
                   "ficha": f"ficha do cliente {k}, com texto que mora no .memo"}
                  for k in range(i + 1, min(i + 5000, n) + 1)]
        r = m({"op": "inserir_lote", "database": "loja", "tabela": "clientes",
               "linhas": linhas})
        if not r.get("ok", True):
            raise SystemExit(f"carga: {r}")
        i += 5000
    return time.perf_counter() - t0


def escrita(m, rotulo, fazer, saida):
    fazer()
    alvo = posicao(m)
    atraso = esperar(alvo)
    retratos = {n: retrato(f) for n, f in C.items()}
    iguais = len(set(retratos.values())) == 1
    ms = -1 if atraso is None else atraso * 1000
    print(f"{rotulo:<32} {alvo:>8}  {ms:>8.0f} ms   "
          f"{'iguais' if iguais else 'DIVERGIRAM: ' + str(retratos)}")
    saida[rotulo] = {"ms": round(ms), "iguais": iguais}


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 100_000
    for nome, porta in PORTAS.items():
        C[nome] = liga(porta)
    m = C["master"]
    r = {}

    print(f"carga inicial: {n} linhas no master")
    t_carga = semear(m, n)
    print(f"  {n / t_carga:,.0f} linhas/s no master (com a imagem no diario)")
    r["master_linhas_s"] = round(n / t_carga)

    t_alcance = esperar(posicao(m))
    print(f"  as tres replicas alcancaram {t_alcance:.1f}s depois do fim da carga")
    print(f"  {n / t_alcance:,.0f} eventos/s por replica, as tres em paralelo")
    r["replica_eventos_s"] = round(n / t_alcance)
    r["alcance_s"] = round(t_alcance, 1)
    print()

    print(f"{'operacao no master':<32} {'diario':>8}  {'ate as 3':>11}   resultado")
    print("-" * 78)
    base = n + 1

    def lote(qtd, inicio):
        linhas = [{"id": k, "nome": f"Cliente {k:07d}", "cidade": "Blumenau",
                   "limite": f"{k}.99", "ficha": f"ficha {k} " * 5}
                  for k in range(inicio, inicio + qtd)]
        return lambda: m({"op": "inserir_lote", "database": "loja",
                          "tabela": "clientes", "linhas": linhas})

    escrita(m, "1 insercao", lote(1, base), r)
    escrita(m, "1.000 insercoes em lote", lote(1000, base + 10), r)
    escrita(m, "1 alteracao", lambda: m({
        "op": "atualizar", "database": "loja", "tabela": "clientes", "rowid": 7,
        "linha": {"id": 7, "nome": "ALTERADO NO MASTER", "cidade": "Bruxelas",
                  "limite": "999.99", "ficha": "ficha trocada, maior que a de antes"}}), r)
    escrita(m, "1 exclusao suave", lambda: m({
        "op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 11,
        "motivo": "prova de replicacao"}), r)
    escrita(m, "1 restauracao", lambda: m({
        "op": "restaurar", "database": "loja", "tabela": "clientes", "rowid": 11,
        "motivo": "voltou"}), r)
    escrita(m, "1 exclusao fisica", lambda: m({
        "op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 13,
        "fisico": True, "motivo": "prova de replicacao fisica"}), r)
    # O anexo e o caso em que copiar o ponteiro daria bloco errado do outro lado.
    escrita(m, "1 linha com memo de 200 KB", lambda: m({
        "op": "inserir", "database": "loja", "tabela": "clientes",
        "linha": {"id": 9_999_999, "nome": "Com anexo", "cidade": "Curitiba",
                  "limite": "1.00", "ficha": "M" * 200_000}}), r)

    print()
    print("QUEDA E RETOMADA do slave03")
    r.update(retomada(m))

    print()
    print(f"{'servidor':<10} {'linhas':>9}  retrato")
    finais = {}
    for nome in PORTAS:
        linhas, digest = retrato(C[nome])
        finais[nome] = digest
        marca = "" if digest == finais["master"] else "  <<< DIFERENTE DO MASTER"
        print(f"{nome:<10} {linhas:>9}  {digest}{marca}")
    r["iguais_no_fim"] = len(set(finais.values())) == 1
    r["linhas"] = linhas

    print()
    print("RESULTADO " + json.dumps(r))


def retomada(m):
    """Derruba o slave03, escreve no master, sobe de volta e cronometra."""
    alvo_pid = None
    for pid in subprocess.run(["pgrep", "-x", "phxsqld"], capture_output=True,
                              text=True).stdout.split():
        cwd = os.path.realpath(f"/proc/{pid}/cwd")
        if cwd.endswith("slave03"):
            alvo_pid = pid
    if alvo_pid is None:
        print("  slave03 nao encontrado -- pulando")
        return {}
    subprocess.run(["kill", alvo_pid], check=False)
    del C["slave03"]
    time.sleep(1)

    n = 4000
    inicio = 20_000_000
    linhas = [{"id": inicio + k, "nome": f"Enquanto caido {k}", "cidade": "Itajai",
               "limite": "10.00", "ficha": "x"} for k in range(n)]
    m({"op": "inserir_lote", "database": "loja", "tabela": "clientes",
       "linhas": linhas})
    alvo = posicao(m)
    print(f"  master gravou {n} linhas com o slave03 derrubado ({alvo} eventos)")

    d = os.path.join(BASE, "slave03")
    log = open(os.path.join(d, "servidor.log"), "a")
    subprocess.Popen(["setsid", PHXSQLD], cwd=d, stdout=log,
                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < 60:
        try:
            C["slave03"] = liga(PORTAS["slave03"])
            break
        except OSError:
            time.sleep(0.05)
    subiu = (time.perf_counter() - t0) * 1000
    while time.perf_counter() - t0 < 300:
        if posicao(C["slave03"]) >= alvo:
            break
        time.sleep(0.05)
    total = time.perf_counter() - t0
    print(f"  voltou a atender em {subiu:.0f} ms e alcancou {n} eventos "
          f"em {total:.1f}s desde o arranque")
    return {"retomada_subiu_ms": round(subiu), "retomada_alcance_s": round(total, 1)}


if __name__ == "__main__":
    main()
