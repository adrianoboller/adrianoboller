#!/usr/bin/env python3
"""O comboio do fecho de janela: quanto o azarado paga pelas tabelas dos outros.

    python3 bancada/concorrencia/o-comboio-do-fecho.py
    SEGUNDOS=8 ESCRITORES=4 python3 bancada/concorrencia/o-comboio-do-fecho.py

A pergunta, e por que nenhum medidor daqui a respondia
-------------------------------------------------------
A §8 do `docs/CONCORRENCIA.md` deixou isto **nomeado e nao medido**: quando a
janela de gravacao fecha, o `gravar_de_verdade` -- que ja roda com a trava
global na mao -- sincroniza a PROPRIA tabela e depois chama
`descarregar_sujas_com`, que **reabre e sincroniza todas as outras sujas** no
mesmo laco. Com K tabelas sujas, o escritor azarado que fecha a janela segura
o servidor inteiro por `K x (open + fsync)`, e os outros K-1 nao pagaram nada.

A premissa foi conferida no fonte antes de virar medicao (`servidor.rs`,
`gravar_de_verdade` -> `descarregar_sujas_com`), porque alvo certo com causa
errada ja custou uma rodada aqui: o pedido 113 mandava ordenar as chaves do
lote e a causa era outra.

O `quanto-a-trava-fica-presa.py` NAO mede isto, e o motivo e o desenho dele:
ele roda com UMA tabela, e com uma tabela o conjunto de sujas fica vazio e o
comboio nunca acontece.

O que se varia, e o que se segura parado
-----------------------------------------
Se eu variasse o numero de escritores, mediria contencao e chamaria de comboio.
Entao o numero de escritores fica FIXO e varia so **em quantas tabelas
distintas eles escrevem**:

    K=1   os 4 escritores todos em `w0`          -> 1 tabela suja
    K=2   dois em `w0`, dois em `w1`             -> 2 tabelas sujas
    K=4   um em cada `w0..w3`                    -> 4 tabelas sujas

Mesmo numero de clientes, mesma carga de escrita, mesmo leitor -- que le
sempre `quieta`, uma tabela que ninguem escreve, para que a espera dele seja
espera de TRAVA e nao de disputa pela propria tabela.

E a media nao serve
--------------------
O comboio acontece **uma vez por janela**, nao por gravacao: diluido na media
de milhares de operacoes ele some (a conta da §8 dava +19,5 us sobre 4.000
gravacoes, enquanto a secao individual segurava 5,2 ms). Por isso aqui se le o
**p99 e o PIOR**, e a media entra so para mostrar que ela nao mostra nada.

So `por_lote`
--------------
Em `por_operacao` toda gravacao sincroniza e a janela nao acumula nada: nao ha
comboio para medir. Medir as duas daria uma tabela bonita e uma coluna vazia.
"""
import json
import multiprocessing as mp
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quieta  # noqa: E402

RAIZ = Path(__file__).resolve().parents[2]
PHXSQLD = Path(os.environ.get("PHX_PHXSQLD", RAIZ / "target/release/phxsqld"))
TOKEN = "o-comboio"
SENHA = "comboio-8765"
SEGUNDOS = float(os.environ.get("SEGUNDOS", "6"))
LINHAS = int(os.environ.get("LINHAS", "500"))
ESCRITORES = int(os.environ.get("ESCRITORES", "4"))
# Os K possiveis sao os divisores do numero de escritores: com 4 escritores em
# 3 tabelas, uma tabela levaria dois e as outras um, e o desequilibrio entraria
# no numero como se fosse efeito do K.
KS = [k for k in (1, 2, 4, 8) if k <= ESCRITORES and ESCRITORES % k == 0]
TABELAS = max(KS)


class Servidor:
    """Um `phxsqld` proprio, morto pelo PID -- nunca por `pkill`, que
    derrubaria o servidor de outra frente na mesma maquina."""

    def __init__(self, porta, durabilidade="por_lote"):
        self.porta = porta
        self.base = Path(f"/tmp/phx-comboio-{os.getpid()}-{porta}")
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        cfg = {
            "base": "dados",
            "bind": f"127.0.0.1:{porta}",
            "token": TOKEN,
            "web": {"ligado": False},
            "recursos": {"durabilidade": durabilidade},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": self.hash_da_senha(SENHA)},
            "usuarios": [],
        }
        (self.base / "config.json").write_text(json.dumps(cfg, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", porta), timeout=2).close()
                return
            except OSError:
                pass
        raise SystemExit(f"o phxsqld nao subiu; veja {self.base}/servidor.log")

    @staticmethod
    def hash_da_senha(senha):
        r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                           capture_output=True, check=True)
        return r.stdout.decode().split('"')[3]

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()
        shutil.rmtree(self.base, ignore_errors=True)


class Cliente:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta), timeout=60)
        self.f = self.s.makefile("rwb")

    def call(self, d, exigir=True):
        d.setdefault("token", TOKEN)
        self.s.sendall((json.dumps(d) + "\n").encode())
        r = json.loads(self.f.readline())
        if exigir and not r.get("ok"):
            raise SystemExit(f"{d['op']} recusado: " + json.dumps(r)[:400])
        return r


def trabalhador(papel, tabela, porta, segundos, fila):
    """PROCESSO e nao thread: com threads a GIL limitaria a vazao e o medidor
    «provaria» a serializacao do servidor medindo a de si mesmo."""
    c = Cliente(porta)
    c.call({"op": "login", "usuario": "root", "senha": SENHA})
    if papel == "ler":
        p = {"op": "varrer", "database": "t", "tabela": tabela, "limite": 50}
    else:
        p = {"op": "inserir", "database": "t", "tabela": tabela,
             "linha": {"nome": "x"}}
    fim = time.monotonic() + segundos
    esperas = []
    while time.monotonic() < fim:
        t0 = time.perf_counter_ns()
        c.call(dict(p), exigir=False)
        esperas.append(time.perf_counter_ns() - t0)
    esperas.sort()
    n = len(esperas)
    q = lambda x: esperas[min(n - 1, int(x * n))] / 1000.0 if n else 0.0
    fila.put({"papel": papel, "ops": n, "media": sum(esperas) / n / 1000.0 if n else 0.0,
              "p50": q(0.50), "p99": q(0.99), "pior": esperas[-1] / 1000.0 if n else 0.0})


def rodada(k, porta, vigia):
    """Um leitor em `quieta` e ESCRITORES escritores repartidos em K tabelas."""
    fila = mp.Queue()
    papeis = [("ler", "quieta")]
    por_tabela = ESCRITORES // k
    for i in range(ESCRITORES):
        papeis.append(("gravar", f"w{i // por_tabela}"))
    procs = [mp.Process(target=trabalhador, args=(p, t, porta, SEGUNDOS, fila))
             for p, t in papeis]
    for p in procs:
        p.start()
    vigia.durante_a_rodada(meus=len(papeis) + 1)
    saidas = [fila.get() for _ in procs]
    for p in procs:
        p.join()
    saiu = {}
    for s in saidas:
        d = saiu.setdefault(s["papel"], {"ops": 0, "media": [], "p50": [],
                                         "p99": [], "pior": 0.0})
        d["ops"] += s["ops"]
        for c in ("media", "p50", "p99"):
            d[c].append(s[c])
        d["pior"] = max(d["pior"], s["pior"])
    for d in saiu.values():
        for c in ("media", "p50", "p99"):
            # A MEDIANA dos quantis dos clientes, e nao a media deles: um
            # cliente que pegou o comboio inteiro nao deve mover o numero dos
            # outros -- ele e o achado, nao o ruido.
            v = sorted(d[c])
            d[c] = v[len(v) // 2]
    return saiu


def semear(c):
    colunas = [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
               {"nome": "nome", "tipo": "Str(20)"}]
    indices = [{"nome": "porId", "colunas": ["id"], "unico": True,
                "primario": True}]
    c.call({"op": "criar_database", "database": "t"}, exigir=False)
    for nome in ["quieta"] + [f"w{i}" for i in range(TABELAS)]:
        c.call({"op": "criar_tabela", "database": "t", "tabela": nome,
                "colunas": colunas, "indices": indices}, exigir=False)
        for _ in range(LINHAS):
            c.call({"op": "inserir", "database": "t", "tabela": nome,
                    "linha": {"nome": "semente"}}, exigir=False)


def principal():
    if not PHXSQLD.exists():
        print(f"falta {PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    sujo_vale = "--mesmo-sujo" in sys.argv
    print("=== o comboio do fecho de janela ===")
    print(f"    {quieta.nucleos()} nucleos | {SEGUNDOS:.0f}s por rodada | "
          f"{ESCRITORES} escritores fixos | K em {KS}")
    print(f"    leitor sempre em `quieta`, que ninguem escreve\n")

    vigia = quieta.Vigia().abrir()
    porta = quieta.porta_livre()
    srv = Servidor(porta, "por_lote")
    medido = {}
    try:
        c = Cliente(porta)
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        semear(c)
        # O controle ANTES: `ping` nao toma a trava de dados. Se ele se mover
        # entre as pontas, quem mudou foi a maquina e a bateria nao compara.
        vigia.controle_antes = so_ping(porta, vigia)
        for k in KS:
            medido[k] = rodada(k, porta, vigia)
        vigia.controle_depois = so_ping(porta, vigia)
    finally:
        srv.parar()
    vigia.fechar()
    vigia.relatar()

    if not vigia.publicavel() and not sujo_vale:
        print("Nenhum numero sai desta rodada. Rode de novo com a maquina")
        print("parada, ou use --mesmo-sujo para depurar o proprio arnes.")
        return 1
    if not vigia.publicavel():
        print(">>> NUMEROS SUJOS: a maquina nao estava parada. NAO CITAR. <<<\n")

    imprimir(medido)
    if "--json" in sys.argv:
        print(json.dumps({"sujo": not vigia.publicavel(),
                          "escritores": ESCRITORES, "medido": medido}, indent=2))
    return 0 if vigia.publicavel() else 1


def so_ping(porta, vigia):
    fila = mp.Queue()

    def bate(fila):
        c = Cliente(porta)
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        fim = time.monotonic() + SEGUNDOS
        n = 0
        t0 = time.monotonic()
        while time.monotonic() < fim:
            c.call({"op": "ping"}, exigir=False)
            n += 1
        fila.put(n / (time.monotonic() - t0))

    p = mp.Process(target=bate, args=(fila,))
    p.start()
    vigia.durante_a_rodada(meus=2)
    v = fila.get()
    p.join()
    return v


def imprimir(medido):
    for papel, titulo in (("gravar", "O ESCRITOR -- quem paga o comboio"),
                          ("ler", "O LEITOR em `quieta` -- quem espera atras")):
        print(f"-- {titulo} (us)")
        print(f"   {'K sujas':>8} {'media':>10} {'p50':>10} {'p99':>10} "
              f"{'PIOR':>11} {'ops/s':>9}")
        base = None
        for k in KS:
            d = medido[k][papel]
            if base is None:
                base = d
            print(f"   {k:>8} {d['media']:>10.1f} {d['p50']:>10.1f} "
                  f"{d['p99']:>10.1f} {d['pior']:>11.1f} "
                  f"{d['ops'] / SEGUNDOS:>9.0f}")
        if base and base["p99"] and base["pior"]:
            fim = medido[KS[-1]][papel]
            print(f"   K={KS[-1]} contra K={KS[0]}:  p99 "
                  f"{fim['p99'] / base['p99']:.2f}x   "
                  f"PIOR {fim['pior'] / base['pior']:.2f}x   "
                  f"media {fim['media'] / base['media']:.2f}x")
        print()


if __name__ == "__main__":
    raise SystemExit(principal())
