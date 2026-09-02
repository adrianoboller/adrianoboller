#!/usr/bin/env python3
"""A trava global SERIALIZA o trabalho? -- a premissa da SP000011, medida.

    python3 bancada/concorrencia/a-trava-serializa.py

Por que este medidor existe antes de a SP000011 comecar
-------------------------------------------------------
A SP000011 e «remocao do `Mutex<Instancia>` global», e ela chega com a
premissa embutida: a trava global custa caro. *Medir a premissa do item vem
antes de implementar o item* -- e esta casa ja errou exatamente este
diagnostico uma vez, ao escrever que «o mutex era o pior pedaco, porque
serializa». Medido, o `lock` sem disputa custava 13,2 ns contra 3.456 us do
parse: 262.000x menos. Diagnostico plausivel nao e diagnostico medido.

O que ele mede, e por que EFEITO e nao ESTADO
----------------------------------------------
Nao pergunta a telemetria «quanto se esperou na fila»: isso e estado, e ja
houve prova nesta casa que passou por engano justamente por conferir estado em
vez de efeito. Aqui se mede a VAZAO TOTAL com N clientes em paralelo:

  * se a trava serializa tudo, N clientes entregam o mesmo que 1  -> ganho 1,0x
  * se nao serializa, a vazao cresce com N                        -> ganho > 1

O confundidor que invalidaria tudo, e o controle
------------------------------------------------
Se os clientes fossem THREADS do Python, a GIL os limitaria e a vazao ficaria
chata mesmo com o servidor perfeitamente paralelo -- o medidor "provaria" a
serializacao do servidor medindo a do proprio medidor. Por isso os clientes
sao PROCESSOS separados (`multiprocessing`), sem GIL comum.

Le e escreve sao medidos SEPARADAMENTE de proposito: serializar escrita e
defensavel; serializar leitura e o que tornaria a SP000011 urgente.

Prova real, nos dois sentidos
-----------------------------
O numero cai sozinho se alguem consertar a trava (o ganho de leitura sobe) ou
se alguem piorar (cai para 1,0x). E o `--nucleos` do cabecalho diz quando a
maquina, e nao a trava, e o teto: com N acima dos nucleos disponiveis o
resultado e um PISO, nao um veredito.
"""
import importlib.util
import json
import multiprocessing as mp
import os
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
os.environ.setdefault("PORTA", "7497")
os.environ.setdefault("PHX_TRABALHO", f"/tmp/phx-trava-{os.getpid()}")

_p = RAIZ / "bancada/dblink/prova-postgres.py"
spec = importlib.util.spec_from_file_location("pp", _p)
pp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pp)

SEGUNDOS = float(os.environ.get("SEGUNDOS", "3"))
LINHAS = int(os.environ.get("LINHAS", "2000"))


def trabalhador(modo, segundos, fila):
    """Um processo cliente: martela ate o prazo e devolve quantas operacoes fez."""
    c = pp.Cliente()
    c.call({"op": "login", "usuario": "root", "senha": pp.SENHA})
    fim = time.monotonic() + segundos
    n = 0
    while time.monotonic() < fim:
        if modo == "sem-trava":
            # O CONTROLE. Mesmo soquete, mesmo JSON, mesmo despacho, mesmo
            # cliente Python -- e `ping` responde SEM tomar `travar_dados`.
            # O que ele nao escalar nao e culpa da trava: e do medidor ou da
            # maquina. A diferenca entre esta curva e a de `ler` E a trava.
            c.call({"op": "ping"}, exigir=False)
        elif modo == "ler":
            c.call({"op": "varrer", "database": "t", "tabela": "c",
                    "limite": 50}, exigir=False)
        else:
            c.call({"op": "inserir", "database": "t", "tabela": "c",
                    "linha": {"nome": "x"}}, exigir=False)
        n += 1
    fila.put(n)


def ocupado_por_cento():
    """Quanto da maquina esta em uso, lido do /proc/stat -- o segundo controle.

    Plateau COM cpu sobrando acusa a trava; plateau com a maquina no teto
    acusa a maquina. Sem este numero, os dois casos sao a mesma curva.
    """
    def amostra():
        campos = [int(x) for x in open("/proc/stat").readline().split()[1:]]
        return sum(campos), campos[3]  # total, ocioso
    t0, i0 = amostra()
    time.sleep(0.4)
    t1, i1 = amostra()
    return 100.0 * (1 - (i1 - i0) / max(1, t1 - t0))


def rodada(modo, n, segundos):
    """N processos em paralelo pelo mesmo prazo -> operacoes por segundo."""
    fila = mp.Queue()
    procs = [mp.Process(target=trabalhador, args=(modo, segundos, fila))
             for _ in range(n)]
    t0 = time.monotonic()
    for p in procs:
        p.start()
    cpu = ocupado_por_cento()  # medida COM a carga rodando, nao antes nem depois
    total = sum(fila.get() for _ in procs)
    for p in procs:
        p.join()
    return total / (time.monotonic() - t0), cpu


def principal():
    if not pp.PHXSQLD.exists():
        print(f"falta {pp.PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    nucleos = os.cpu_count() or 1
    srv = pp.Phxsqld()
    try:
        c = pp.Cliente()
        c.call({"op": "login", "usuario": "root", "senha": pp.SENHA})
        c.call({"op": "criar_database", "database": "t"})
        c.call({"op": "criar_tabela", "database": "t", "tabela": "c",
                "colunas": [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                            {"nome": "nome", "tipo": "Str(20)"}],
                "indices": [{"nome": "porId", "colunas": ["id"],
                             "unico": True, "primario": True}]})
        # A tabela nasce com dado: varrer tabela vazia nao toca no que importa.
        for _ in range(LINHAS):
            c.call({"op": "inserir", "database": "t", "tabela": "c",
                    "linha": {"nome": "semente"}}, exigir=False)

        print(f"=== a trava global serializa? ===")
        print(f"    nucleos: {nucleos} | {SEGUNDOS:.0f}s por rodada | "
              f"{LINHAS} linhas semeadas\n")
        saida = {"nucleos": nucleos, "segundos": SEGUNDOS, "linhas": LINHAS}
        for modo in ("sem-trava", "ler", "gravar"):
            print(f"-- {modo}")
            base = None
            saida[modo] = {}
            for n in (1, 2, 4):
                ops, cpu = rodada(modo, n, SEGUNDOS)
                if base is None:
                    base = ops
                ganho = ops / base
                teto = "  (N > nucleos: PISO, nao veredito)" if n > nucleos - 1 else ""
                print(f"   {n} cliente(s): {ops:8.0f} op/s   ganho {ganho:.2f}x"
                      f"   cpu {cpu:3.0f}%{teto}")
                saida[modo][n] = {"ops": ops, "ganho": ganho, "cpu": cpu}
            print()
        print(json.dumps(saida, indent=2))
        return 0
    finally:
        srv.parar()


if __name__ == "__main__":
    raise SystemExit(principal())
