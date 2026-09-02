#!/usr/bin/env python3
"""Quanto tempo uma VARREDURA segura a trava global -- e quanto isso custa a quem espera.

    python3 bancada/concorrencia/custo-da-varredura.py

# A pergunta, e de onde ela veio

O mapa da concorrencia contou **27 secoes criticas de leitura com varredura**,
1.795 linhas. Elas viraram as candidatas a primeiro alvo depois que a premissa
das cinco secoes de gatilho MORREU MEDIDA: o teto de passos existe e vale
**18,3 ms** no pior caso.

Mas trocar um palpite por outro nao e medir. As 27 nunca foram medidas em
TEMPO DE TRAVA, e e isso que este arnes responde.

# O que ele mede, e por que o `pular` e o interessante

A pagina que a operacao DEVOLVE e limitada pelo `max_linhas`. O que NAO e
limitado e o caminho ate ela: `pular` (o `OFFSET` do SQL) anda linha a linha
com a trava na mao ate chegar a posicao, e so entao monta a pagina. O proprio
comentario do `Table::pagina` diz que ele e o «modo de compatibilidade», e que
tabela grande usa cursor -- este medidor poe numero nessa frase.

Tres caminhos, na MESMA tabela:

  1. primeira pagina (pular 0)      -- o barato, o que a tela faz
  2. pagina no meio (pular grande)  -- o `OFFSET`, que anda ate la
  3. contar                          -- que sai do cabecalho, sem varrer

# O controle, e o numero que importa de verdade

Enquanto A varre, B manda `ping` num laco e mede a propria espera. `ping` nao
toca em tabela nenhuma: se ele espera, foi a TRAVA que o segurou -- e a pior
espera do B e o custo real da varredura para o resto do servidor.

A linha de base do B e medida com A parado. Sem ela o numero do B nao quer
dizer nada: parte da espera e o soquete e o JSON, e nao a trava.

O `quieta.Vigia` mede a maquina nas pontas e RECUSA publicar numero sujo.
"""
import importlib.util
import json
import statistics
import sys
import threading
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
import quieta  # noqa: E402

_spec = importlib.util.spec_from_file_location("desenho", AQUI / "escolher-o-desenho.py")
desenho = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(desenho)

def entrar(c):
    """Todo cliente entra: com usuario cadastrado, o token sozinho nao basta."""
    c.call({"op": "login", "usuario": "root", "senha": desenho.SENHA})


LINHAS = 50_000
LOTE = 5_000
PAGINA = 1_000


def semear(c, quantas):
    entrar(c)
    c.call({"op": "criar_database", "database": "b"})
    c.call({"op": "criar_tabela", "database": "b", "tabela": "grande",
            "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                        {"nome": "nome", "tipo": "Str(40)"},
                        {"nome": "cidade", "tipo": "Str(30)"},
                        # A COLUNA EXTERNA e o experimento: ela mora no `.memo`,
                        # e so paga leitura quem decodificar COM anexos. Uma
                        # tabela sem ela nao distingue os dois caminhos -- foi
                        # por isso que a primeira medicao nao viu a regressao.
                        {"nome": "ficha", "tipo": "Memo"}],
            "indices": [{"nome": "pk", "colunas": ["id"], "unico": True, "primario": True}]})
    print(f"  semeando {quantas:,} linhas".replace(",", "."), end="", flush=True)
    t0 = time.time()
    for base in range(0, quantas, LOTE):
        linhas = [{"id": base + i, "nome": f"nome {base + i}", "cidade": "Blumenau",
                   "ficha": "x" * 400}
                  for i in range(1, min(LOTE, quantas - base) + 1)]
        c.call({"op": "inserir_lote", "database": "b", "tabela": "grande", "linhas": linhas})
        print(".", end="", flush=True)
    print(f" {time.time() - t0:.1f}s")


class Vizinho(threading.Thread):
    """O B: bate no servidor num laco e mede a PROPRIA espera.

    DOIS SABORES, e a diferenca entre eles e o experimento inteiro:

      * `ping` **nao toma a trava de dados** -- ele responde direto no
        despachar (`servidor.rs:6059`). Serve de CONTROLE: mede o custo do
        soquete, do JSON e do agendador, e mais nada.
      * `ler` **toma** (`travar_dados()` logo na entrada do `op_ler`). Ele e o
        que sente a trava, e a espera dele e o custo real da varredura para o
        resto do servidor.

    A primeira versao deste arnes usava so o `ping`, e por isso quase publicou
    «a varredura de 357 ms nao atrapalha ninguem»: o instrumento nao passava
    pelo que ele deveria estar medindo. Controle serve para DESCONTAR o custo
    que nao e da trava -- nao para medir a espera POR ela.
    """

    def __init__(self, porta, op):
        super().__init__(daemon=True)
        self.porta, self.op, self.paradas, self.parar = porta, op, [], False

    def pedido(self):
        if self.op == "ping":
            return {"op": "ping"}
        return {"op": "ler", "database": "b", "tabela": "pequena", "rowid": 1}

    def run(self):
        c = desenho.Cliente(self.porta)
        entrar(c)
        while not self.parar:
            t0 = time.perf_counter()
            c.call(self.pedido(), exigir=False)
            self.paradas.append((time.perf_counter() - t0) * 1000)


def medir(porta, rotulo, pedido, bases):
    """Roda a varredura com DOIS vizinhos, e devolve a pior espera de cada um."""
    vs = [Vizinho(porta, "ping"), Vizinho(porta, "ler")]
    for v in vs:
        v.start()
    time.sleep(0.5)                      # os vizinhos pegam ritmo antes de A
    marcas = [len(v.paradas) for v in vs]
    a = desenho.Cliente(porta)
    entrar(a)
    t0 = time.perf_counter()
    r = a.call(pedido, exigir=False)
    dur = (time.perf_counter() - t0) * 1000
    for v in vs:
        v.parar = True
    for v in vs:
        v.join(timeout=5)
    piores = [max(v.paradas[m:] or [0.0]) for v, m in zip(vs, marcas)]
    devolvidas = len(r.get("resultado", {}).get("linhas", [])) if r.get("ok") else -1
    print(f"  {rotulo:<32} {dur:8.1f} ms  |  ping {piores[0]:7.1f} ms"
          f"  |  ler {piores[1]:7.1f} ms  ({piores[1] / bases[1]:5.0f}x)"
          f"  | linhas {devolvidas}")
    return dur, piores


def principal():
    if not desenho.PHXSQLD.exists():
        return print(f"falta {desenho.PHXSQLD}") or 2
    vigia = quieta.Vigia().abrir()
    porta = quieta.porta_livre()
    srv = desenho.Servidor(porta)
    try:
        c = desenho.Cliente(porta)
        semear(c, LINHAS)

        # A TABELA DO VIZINHO. Pequena de proposito: o `ler` dele tem de
        # custar quase nada quando a trava esta livre, senao a espera que ele
        # mede seria a dele mesmo.
        c.call({"op": "criar_tabela", "database": "b", "tabela": "pequena",
                "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True}],
                "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                             "primario": True}]})
        c.call({"op": "inserir", "database": "b", "tabela": "pequena",
                "linha": {"id": 1}})

        # AS DUAS LINHAS DE BASE, com ninguem varrendo. Sem elas a espera dos
        # vizinhos nao se distingue do custo do soquete e do JSON.
        bases = []
        for op in ("ping", "ler"):
            v = Vizinho(porta, op)
            v.start(); time.sleep(1.5); v.parar = True; v.join(timeout=5)
            bases.append(statistics.median(v.paradas))
        print(f"\n  base com a maquina em repouso:  ping {bases[0]:.3f} ms   "
              f"ler {bases[1]:.3f} ms")
        print("  (o `ping` NAO toma a trava; o `ler` toma -- a diferenca entre as"
              "\n   duas colunas abaixo e o custo da trava, e nao do soquete)\n")
        print(f"  {'a varredura':<32} {'dura':>8}     "
              f"|  {'pior ping':>9}  |  {'pior ler':>9}")

        medir(porta, "varrer, primeira pagina",
              {"op": "varrer", "database": "b", "tabela": "grande", "max": PAGINA}, bases)
        for pular in (10_000, 20_000, 40_000):
            medir(porta, f"varrer, pular {pular:,}".replace(",", "."),
                  {"op": "varrer", "database": "b", "tabela": "grande",
                   "max": PAGINA, "pular": pular}, bases)
        medir(porta, "varrer por INDICE, pular 20.000",
              {"op": "varrer", "database": "b", "tabela": "grande", "indice": "pk",
               "max": PAGINA, "pular": 20_000}, bases)

        vigia.durante_a_rodada()
        motivos = vigia.fechar().motivos()
        if motivos:
            print("\n  NUMERO SUJO -- a maquina nao estava quieta:")
            for m in motivos:
                print("   ·", m)
            print("  Os numeros acima NAO valem. Repita com a maquina parada.")
        else:
            print("\n  a maquina esteve quieta nas duas pontas: os numeros valem")
    finally:
        srv.parar()
    return 0


if __name__ == "__main__":
    sys.exit(principal())
