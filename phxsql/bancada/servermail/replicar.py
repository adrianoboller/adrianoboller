#!/usr/bin/env python3
"""PASSO 5: replicacao master -> 3 slaves, provada pelo SHA-256 por linha.

    python3 bancada/servermail/replicar.py <dir> [n_linhas]

Reusa a montagem de `bancada/replicacao/montar.py` (master 5800 + slave01/02/03),
mas NAO usa o `medir.py`: aquele mede vazao e atraso e assume que a tabela ja
existe na replica quando pergunta a `posicao` -- e num arranque frio a replica
pode ainda nao te-la criado, e o poller estoura com KeyError. Aqui o proposito e
outro e mais simples: provar que o dado chega IDENTICO. Entao a espera tolera a
tabela ainda nao existir, e a prova e o retrato SHA-256 de cada linha inteira
(o mesmo criterio do medir.py), que so se compara quando os quatro convergem.

Sobe e derruba pela propria bancada de replicacao (por caminho, nunca por nome).
Ultima linha: `RESULTADO <json>`.
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
MONTAR = os.path.join(RAIZ, "bancada", "replicacao", "montar.py")
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


def eventos(fala):
    """Eventos aplicados na tabela clientes -- ou -1 se ela ainda nao existe."""
    r = fala({"op": "posicao", "database": "loja"})
    if not r.get("ok"):
        return -1
    tab = r["resultado"].get("tabelas", {}).get("clientes")
    return tab["eventos"] if tab else -1


def retrato(fala):
    h = hashlib.sha256()
    linhas, depois = 0, 0
    while True:
        r = fala({"op": "varrer", "database": "loja", "tabela": "clientes",
                  "max": 2000, "depois": depois, "visao": "todas"})
        d = r["resultado"]
        for l in d["linhas"]:
            h.update(json.dumps(l, sort_keys=True, ensure_ascii=False).encode())
            linhas += 1
        if not d["ha_mais"] or not d["linhas"]:
            break
        depois = d["cursor_fim"]
    return linhas, h.hexdigest()[:16]


def semear(m, n):
    m({"op": "criar_database", "database": "loja"})
    m({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                   {"nome": "cidade", "tipo": "Str(30)"},
                   {"nome": "ficha", "tipo": "Memo"}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    cid = ["Blumenau", "Joinville", "Itajai", "Curitiba", "Florianopolis"]
    i = 0
    while i < n:
        linhas = [{"id": k, "nome": f"Cliente {k:07d}", "cidade": cid[k % 5],
                   "ficha": f"ficha do cliente {k} no .memo"}
                  for k in range(i + 1, min(i + 5000, n) + 1)]
        r = m({"op": "inserir_lote", "database": "loja", "tabela": "clientes",
               "linhas": linhas})
        if not r.get("ok", True):
            raise SystemExit(f"carga: {r}")
        i += 5000


def esperar_convergir(conns, alvo, prazo=180):
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < prazo:
        pos = {n: eventos(f) for n, f in conns.items()}
        if all(v >= alvo for v in pos.values()):
            return time.perf_counter() - t0, pos
        time.sleep(0.1)
    return None, {n: eventos(f) for n, f in conns.items()}


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "/tmp/phx-rep-servermail"
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 20000
    out = {"linhas": n, "quando": time.strftime("%Y-%m-%d")}

    subprocess.run([sys.executable, MONTAR, "--derrubar", base],
                   capture_output=True)
    subprocess.run(["rm", "-rf", base], check=False)
    r = subprocess.run([sys.executable, MONTAR, base], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"montar falhou: {r.stderr}")
    try:
        time.sleep(1.0)
        conns = {name: liga(p) for name, p in PORTAS.items()}
        print(f"carga inicial: {n} linhas no master")
        t_carga = time.perf_counter()
        semear(conns["master"], n)
        out["master_s"] = round(time.perf_counter() - t_carga, 2)
        out["master_linhas_s"] = round(n / out["master_s"]) if out["master_s"] else None

        t_alc, pos = esperar_convergir(conns, n)
        out["alcance_s"] = round(t_alc, 2) if t_alc is not None else None
        out["eventos_por_no"] = pos
        print(f"alcance das 3 replicas: {out['alcance_s']}s  posicoes={pos}")

        retratos = {name: retrato(f) for name, f in conns.items()}
        digs = {v[1] for v in retratos.values()}
        out["retratos"] = {k: {"linhas": v[0], "sha256": v[1]} for k, v in retratos.items()}
        out["iguais_no_fim"] = (len(digs) == 1) and all(v >= n for v in pos.values())
        for name, (lin, dig) in retratos.items():
            print(f"  {name:8} linhas={lin:6} sha256[:16]={dig}")
        print(f"RETRATOS IDENTICOS NOS QUATRO: {out['iguais_no_fim']} -> {digs}")
    finally:
        subprocess.run([sys.executable, MONTAR, "--derrubar", base],
                       capture_output=True)
        subprocess.run(["rm", "-rf", base], check=False)

    print("\nRESULTADO " + json.dumps(out, ensure_ascii=False))
    return 0 if out.get("iguais_no_fim") else 1


if __name__ == "__main__":
    sys.exit(main())
