#!/usr/bin/env python3
"""Bancada do pedido 393: o braco do `unir` como PEDIDO.

Mede duas coisas contra o motor vivo, pelo soquete, e grava
`resultados.json` com a data DENTRO do arquivo -- nunca do `mtime`.

1. **Trabalho igual, dois caminhos.** A mesma resposta para
   `SELECT id,nome FROM g1 WHERE uf='SC' UNION SELECT id,nome FROM g2
   WHERE uf='SC'`: (a) `unir` por `tabelas`, que materializa as duas
   INTEIRAS e obriga quem perguntou a filtrar fora; (b) `unir` por
   `partes`, com um `buscar` por indice DENTRO de cada braco. A sonda
   confere que as duas respostas sao IGUAIS antes de comparar tempo --
   a regra 3 do `bancada/LEIA-ME.md`.

2. **O que um leitor inocente espera.** Um `varrer(max=1)` numa conexao
   vizinha, sozinho e sob cada um dos dois caminhos, numa JANELA FIXA:
   sem janela fixa «pior espera baixa» quer dizer so «a carga acabou
   antes», e o caminho `partes` acaba ~56x mais rapido.

Publica mediana com faixa min-max, e **nao declara vencedor quando as
faixas se cruzam** (pedido 155).

    bancada/esta-medindo.sh && echo espere
    cargo build --release --bin phxsqld     # binario velho mede o passado
    python3 bancada/uniao/medir.py
"""
import json
import os
import shutil
import socket
import statistics
import subprocess
import threading
import time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARIO = os.path.join(RAIZ, "target", "release", "phxsqld")
SAIDA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resultados.json")
PORTA = int(os.environ.get("PHX_PORTA", "6187"))
BASE = f"/tmp/phx-bancada-uniao-{os.getpid()}"
N = 20000
REPETICOES = 7
CORRIDAS = 3
JANELA_S = 1.5
RODADAS_DA_JANELA = 3


class Conn:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta), 60)
        self.f = self.s.makefile("rwb")

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        return r.get("resultado") if r.get("ok") else {"erro": r.get("erro") or r}

    def close(self):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass


def faixa(v):
    return f"{statistics.median(v):.1f} ms ({min(v):.1f}-{max(v):.1f})"


def cruzam(a, b):
    return not (max(a) < min(b) or max(b) < min(a))


BRACOS = [{"op": "buscar", "tabela": "g1", "indice": "porUf", "chave": "SC", "max": 5000},
          {"op": "buscar", "tabela": "g2", "indice": "porUf", "chave": "SC", "max": 5000}]


def subir():
    shutil.rmtree(BASE, ignore_errors=True)
    os.makedirs(BASE + "/dados", exist_ok=True)
    cfg = BASE + "/config.json"
    json.dump({"bind": f"127.0.0.1:{PORTA}", "cifra_fio": {"exigir": False},
               "base": BASE + "/dados", "token": "t", "web": {"ligado": False}},
              open(cfg, "w"))
    p = subprocess.Popen([BINARIO, "--config", cfg], cwd=RAIZ,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(200):
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
            return p
        except OSError:
            time.sleep(0.1)
    p.terminate()
    raise SystemExit("o servidor nao subiu")


def carregar(pedir):
    pedir(op="criar_database", database="loja")
    for t in ("g1", "g2"):
        r = pedir(op="criar_tabela", database="loja", tabela=t,
                  colunas=[{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                           {"nome": "uf", "tipo": "Str(2)"},
                           {"nome": "nome", "tipo": "Str(40)"}],
                  indices=[{"nome": "porId", "colunas": ["id"], "unico": True,
                            "primario": True},
                           {"nome": "porUf", "colunas": ["uf"]}])
        if r.get("erro"):
            raise SystemExit("criar_tabela: " + str(r["erro"]))
        base = 0 if t == "g1" else N
        # 1% de seletividade: e ela que decide o tamanho do ganho, e por
        # isso ela e declarada aqui em vez de sair de um sorteio.
        r = pedir(op="inserir_lote", database="loja", tabela=t,
                  linhas=[{"id": base + i, "uf": ("SC" if i % 100 == 0 else "SP"),
                           "nome": f"n{base+i:06d}"} for i in range(1, N + 1)])
        if r.get("erro"):
            raise SystemExit("inserir_lote: " + str(r["erro"]))


def coluna(r, nome):
    return [c.get("nome") for c in r.get("colunas", [])].index(nome)


def main():
    p = subir()
    c = Conn(PORTA)
    pedir = c.pedir
    saida = {"medido_em": time.strftime("%Y-%m-%d %H:%M"),
             "carga_da_maquina": open("/proc/loadavg").read().split()[0],
             "binario_de": time.strftime("%Y-%m-%d %H:%M",
                                         time.localtime(os.path.getmtime(BINARIO))),
             "linhas_por_tabela": N, "seletividade_pct": 1}
    try:
        carregar(pedir)

        # ---- 1. trabalho igual, dois caminhos
        razoes, tudo_a, tudo_b = [], [], []
        for _ in range(CORRIDAS):
            ta, tb = [], []
            for _ in range(REPETICOES):
                t0 = time.perf_counter()
                u = pedir(op="unir", database="loja", tabelas=["g1", "g2"],
                          modo="distinta")
                i, uf, n = coluna(u, "id"), coluna(u, "uf"), coluna(u, "nome")
                fora = sorted({(l[i], l[n]) for l in u["linhas"] if l[uf] == "SC"})
                ta.append((time.perf_counter() - t0) * 1000)
            for _ in range(REPETICOES):
                t0 = time.perf_counter()
                v = pedir(op="unir", database="loja", modo="distinta", partes=BRACOS)
                if v.get("erro"):
                    raise SystemExit("o braco-pedido recusou: " + str(v["erro"])[:300])
                i, n = coluna(v, "id"), coluna(v, "nome")
                dentro = sorted((l[i], l[n]) for l in v["linhas"])
                tb.append((time.perf_counter() - t0) * 1000)
            if fora != dentro:
                raise SystemExit(f"respostas diferentes: {len(fora)} x {len(dentro)}"
                                 " -- a comparacao nao vale")
            razoes.append(statistics.median(ta) / statistics.median(tb))
            tudo_a += ta
            tudo_b += tb
            print(f"  corrida: (a) {faixa(ta)}   (b) {faixa(tb)}   "
                  f"{razoes[-1]:.1f}x")

        saida.update({
            "linhas_na_resposta": len(dentro),
            "linhas_materializadas_por_tabelas": u.get("quantas"),
            "linhas_materializadas_por_partes": v.get("quantas"),
            "tabelas_e_filtro_fora_ms": round(statistics.median(tudo_a), 1),
            "tabelas_e_filtro_fora_faixa": [round(min(tudo_a), 1), round(max(tudo_a), 1)],
            "partes_filtro_dentro_ms": round(statistics.median(tudo_b), 1),
            "partes_filtro_dentro_faixa": [round(min(tudo_b), 1), round(max(tudo_b), 1)],
            "razao": round(statistics.median(razoes), 1),
            "razao_por_corrida": [round(x, 1) for x in razoes],
            "faixas_se_cruzam": cruzam(tudo_a, tudo_b),
            "mesma_resposta": True,
        })
        print(f"\n  (a) tabelas + filtro fora : {faixa(tudo_a)}  "
              f"o motor carregou {u.get('quantas')}")
        print(f"  (b) partes, filtro dentro : {faixa(tudo_b)}  "
              f"o motor carregou {v.get('quantas')}")
        print(f"  razao {saida['razao']}x -- faixas se cruzam? "
              + ("SIM: SEM VENCEDOR" if saida["faixas_se_cruzam"] else "NAO"))

        # ---- 2. o leitor inocente, em janela FIXA
        parado, fim = [], threading.Event()

        def sondador():
            s2 = Conn(PORTA)
            while not fim.is_set():
                t0 = time.perf_counter()
                s2.pedir(op="varrer", database="loja", tabela="g1", max=1)
                parado.append((time.perf_counter() - t0) * 1000)
                time.sleep(0.002)
            s2.close()

        def rodar(carga):
            p95s, maxs, quantas = [], [], []
            for _ in range(RODADAS_DA_JANELA):
                parado.clear()
                fim.clear()
                th = threading.Thread(target=sondador)
                th.start()
                t0, q = time.perf_counter(), 0
                while time.perf_counter() - t0 < JANELA_S:
                    if carga is None:
                        time.sleep(0.05)
                    else:
                        carga()
                        q += 1
                fim.set()
                th.join()
                v = sorted(parado)
                p95s.append(v[min(int(len(v) * .95), len(v) - 1)])
                maxs.append(max(v))
                quantas.append(q)
            return p95s, maxs, quantas

        for rotulo, carga in (
            ("sozinho", None),
            ("tabelas", lambda: pedir(op="unir", database="loja",
                                      tabelas=["g1", "g2"], modo="tudo")),
            ("partes", lambda: pedir(op="unir", database="loja", modo="tudo",
                                     partes=BRACOS)),
        ):
            p95s, maxs, quantas = rodar(carga)
            saida[f"leitor_{rotulo}_p95_ms"] = round(statistics.median(p95s), 2)
            saida[f"leitor_{rotulo}_p95_faixa"] = [round(min(p95s), 2), round(max(p95s), 2)]
            saida[f"leitor_{rotulo}_max_ms"] = round(statistics.median(maxs), 2)
            saida[f"unioes_na_janela_{rotulo}"] = quantas
            print(f"  varrer(max=1) {rotulo:<8}: p95 {statistics.median(p95s):.2f} ms "
                  f"({min(p95s):.2f}-{max(p95s):.2f})  unioes: {quantas}")

        saida["leitor_faixas_se_cruzam"] = cruzam(
            saida["leitor_tabelas_p95_faixa"], saida["leitor_partes_p95_faixa"])
        saida["leitor_razao_p95"] = round(
            saida["leitor_tabelas_p95_ms"] / saida["leitor_partes_p95_ms"], 1)
        saida["prova"] = (
            "pelo SOQUETE, com a MESMA resposta conferida antes de comparar "
            "tempo, e o leitor vizinho medido em janela fixa de "
            f"{JANELA_S} s de pressao continua")
        json.dump(saida, open(SAIDA, "w"), indent=1, ensure_ascii=False)
        print("\ngravado em", SAIDA)
    finally:
        c.close()
        p.terminate()
        p.wait(timeout=10)
        shutil.rmtree(BASE, ignore_errors=True)


if __name__ == "__main__":
    main()
