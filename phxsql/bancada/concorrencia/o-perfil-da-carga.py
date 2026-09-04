#!/usr/bin/env python3
"""O teto do `RwLock` NAO e um numero: e uma funcao do formato da carga.

    python3 bancada/concorrencia/o-perfil-da-carga.py
    SEGUNDOS=6 LIMITES=1,5,50 python3 bancada/concorrencia/o-perfil-da-carga.py

A ressalva que virou medicao
-----------------------------
A §3.1 do `docs/CONCORRENCIA.md` carrega esta ressalva desde 03/09, e ela
nunca foi medida:

  «Os numeros comparam um `varrer` de 50 linhas contra um `inserir` de uma.
  Nessa forma a leitura custa 20x mais por operacao e portanto segura a trava
  20x mais tempo -- o que favorece o `RwLock` POR CONSTRUCAO. Isso nao invalida
  a medicao; invalida generaliza-la.»

A §11 mediu o teto do `RwLock` em 2,48x-2,99x e recomendou o desenho com base
nele. Se esse 2,5x so existe porque a leitura escolhida e cara, a recomendacao
vale para uma carga e nao para o produto -- e quem a ler daqui a tres meses
nao vai saber disso.

Entao esta bancada varia UMA coisa: **quanto trabalho uma leitura faz**
(`varrer` com limite 1, 5 e 50), com a escrita parada em `inserir` de uma
linha. E ela nao usa o limite nominal como eixo: usa a **razao de custo
medida** entre uma leitura e uma escrita, porque e essa razao que a §3.1
nomeia, e limite nao e custo.

O que se espera, e o que seria o achado
----------------------------------------
Se o teto do `RwLock` cair para perto de 1,0x quando a leitura fica barata, a
§11 esta certa e INCOMPLETA: o desenho compra muito numa carga de leitura
pesada e quase nada numa de leitura leve. Se o teto se segurar nos tres, a
recomendacao generaliza e a ressalva da §3.1 pode ser aposentada com numero.

Os dois resultados valem. *Hipotese que morre medida e resultado tao valido
quanto ganho.*
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
TOKEN = "o-perfil"
SENHA = "perfil-2468"
SEGUNDOS = float(os.environ.get("SEGUNDOS", "6"))
LINHAS = int(os.environ.get("LINHAS", "2000"))
LIMITES = [int(x) for x in os.environ.get("LIMITES", "1,5,50").split(",")]
CLIENTES = [1, 4]


class Servidor:
    """Morto pelo PID, nunca por `pkill`: matar por nome derrubaria o servidor
    de outra frente na mesma maquina, e isso ja aconteceu aqui."""

    def __init__(self, porta):
        self.porta = porta
        self.base = Path(f"/tmp/phx-perfil-{os.getpid()}-{porta}")
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        cfg = {
            "base": "dados",
            "bind": f"127.0.0.1:{porta}",
            "token": TOKEN,
            "web": {"ligado": False},
            "recursos": {"durabilidade": "por_lote"},
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


def pedido(perfil, limite):
    if perfil == "controle":
        # `ping` responde SEM tomar a trava de dados: o que ele nao escalar nao
        # e culpa da trava, e do medidor ou da maquina.
        return {"op": "ping"}
    if perfil == "ler":
        return {"op": "varrer", "database": "t", "tabela": "c",
                "max": limite}
    return {"op": "inserir", "database": "t", "tabela": "c",
            "linha": {"nome": "x"}}


def trabalhador(perfil, limite, porta, segundos, fila):
    c = Cliente(porta)
    c.call({"op": "login", "usuario": "root", "senha": SENHA})
    p = pedido(perfil, limite)
    fim = time.monotonic() + segundos
    n = 0
    t0 = time.monotonic()
    while time.monotonic() < fim:
        c.call(dict(p), exigir=False)
        n += 1
    fila.put(n / (time.monotonic() - t0))


def rodada(perfil, limite, n, porta, vigia):
    fila = mp.Queue()
    procs = [mp.Process(target=trabalhador,
                        args=(perfil, limite, porta, SEGUNDOS, fila))
             for _ in range(n)]
    for p in procs:
        p.start()
    vigia.durante_a_rodada(meus=n + 1)
    total = sum(fila.get() for _ in procs)
    for p in procs:
        p.join()
    return total


def semear(c):
    colunas = [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
               {"nome": "nome", "tipo": "Str(20)"}]
    indices = [{"nome": "porId", "colunas": ["id"], "unico": True,
                "primario": True}]
    c.call({"op": "criar_database", "database": "t"}, exigir=False)
    c.call({"op": "criar_tabela", "database": "t", "tabela": "c",
            "colunas": colunas, "indices": indices}, exigir=False)
    for _ in range(LINHAS):
        c.call({"op": "inserir", "database": "t", "tabela": "c",
                "linha": {"nome": "semente"}}, exigir=False)


def principal():
    if not PHXSQLD.exists():
        print(f"falta {PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    sujo_vale = "--mesmo-sujo" in sys.argv
    print("=== o teto do RwLock contra o FORMATO da carga ===")
    print(f"    {quieta.nucleos()} nucleos | {SEGUNDOS:.0f}s por rodada | "
          f"varrer com limite em {LIMITES}, contra inserir de 1 linha\n")

    vigia = quieta.Vigia().abrir()
    porta = quieta.porta_livre()
    srv = Servidor(porta)
    med = {}
    try:
        c = Cliente(porta)
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        semear(c)
        # A GUARDA, antes de qualquer numero: voltou o que se pediu?
        quieta.confira_a_pagina(c.call, lambda n: pedido("ler", n))
        # O controle nao depende do limite: mede-se UMA vez por contagem de
        # clientes, e nao uma por limite -- repeti-lo so somaria ruido.
        ctrl = {n: rodada("controle", 0, n, porta, vigia) for n in CLIENTES}
        vigia.controle_antes = ctrl[1]
        escrita1 = rodada("gravar", 0, 1, porta, vigia)
        for lim in LIMITES:
            med[lim] = {n: rodada("ler", lim, n, porta, vigia)
                        for n in CLIENTES}
        vigia.controle_depois = rodada("controle", 0, 1, porta, vigia)
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

    imprimir(ctrl, escrita1, med)
    if "__json__" in sys.argv or "--json" in sys.argv:
        print(json.dumps({"sujo": not vigia.publicavel(), "controle": ctrl,
                          "escrita_1_cliente": escrita1, "leitura": med},
                         indent=2))
    return 0 if vigia.publicavel() else 1


def imprimir(ctrl, escrita1, med):
    n = CLIENTES[-1]
    ganho_ctrl = ctrl[n] / ctrl[1] if ctrl[1] else 0.0
    print(f"-- o controle (`ping`, que nao toma a trava): "
          f"{ctrl[1]:.0f} -> {ctrl[n]:.0f} op/s, ganho {ganho_ctrl:.2f}x")
    print(f"-- a escrita com 1 cliente: {escrita1:.0f} op/s\n")
    print(f"   {'limite':>7} {'ler 1cli':>10} {'ler 4cli':>10} {'ganho':>8} "
          f"{'razao ler/gravar':>18} {'teto RwLock':>13}")
    for lim in LIMITES:
        um, quatro = med[lim][1], med[lim][n]
        ganho = quatro / um if um else 0.0
        # A razao de CUSTO por operacao, que e o eixo que a §3.1 nomeia:
        # quantas vezes uma leitura custa mais que uma escrita. Vazao menor =
        # custo maior, entao a razao e o inverso das vazoes.
        razao = escrita1 / um if um else 0.0
        teto = ganho_ctrl / ganho if ganho else 0.0
        print(f"   {lim:>7} {um:>10.0f} {quatro:>10.0f} {ganho:>7.2f}x "
              f"{razao:>17.1f}x {teto:>12.2f}x")
    print()


if __name__ == "__main__":
    raise SystemExit(principal())
