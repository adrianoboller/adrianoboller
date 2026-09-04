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
se alguem piorar (cai para 1,0x). E o cabecalho diz quantos nucleos ESTE
processo alcanca: com N acima deles o resultado e um PISO, nao um veredito.

E a maquina precisa estar parada -- senao ele RECUSA
----------------------------------------------------
Este medidor tem um modo de falhar que nao da erro: numa maquina ocupada a
curva achata, e a curva achatada e exatamente o sintoma que se esperava da
trava. O ruido aponta PARA O MESMO LADO que a hipotese, e sai com casas
decimais.

Por isso o `quieta.Vigia` mede a maquina nas duas pontas e durante cada
rodada, roda a curva de controle no comeco E no fim, e quando alguma das tres
se mexeu **esta bateria nao imprime numero nenhum**. Publicar sujo com uma
ressalva ao lado nao resolve: tres documentos adiante a ressalva ficou para
tras e o numero virou fato. O `--mesmo-sujo` existe so para depurar o proprio
arnes, e carimba tudo o que sai.

Qual desenho substitui a trava e OUTRA medicao, e ela mora ao lado, no
`escolher-o-desenho.py`: este aqui responde «a trava custa?», e nao «o que
por no lugar?».
"""
import importlib.util
import json
import multiprocessing as mp
import os
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
sys.path.insert(0, str(AQUI))
import quieta  # noqa: E402

# A porta vem da FAIXA desta frente e e escolhida LIVRE, nao fixada. Fixa em
# 7497 -- como estava -- ela caia fora da faixa reservada aqui e podia subir
# por cima do soquete de outra frente medindo ao lado; e duas medicoes de
# concorrencia dividindo maquina estragam as duas.
os.environ.setdefault("PORTA", str(quieta.porta_livre()))
os.environ.setdefault("PHX_TRABALHO", f"/tmp/phx-trava-{os.getpid()}")

_p = RAIZ / "bancada/dblink/prova-postgres.py"
spec = importlib.util.spec_from_file_location("pp", _p)
pp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pp)

SEGUNDOS = float(os.environ.get("SEGUNDOS", "3"))
LINHAS = int(os.environ.get("LINHAS", "2000"))


def trabalhador(modo, segundos, fila, tabela="c"):
    """Um processo cliente: martela ate o prazo e devolve quantas operacoes fez.

    `tabela` existe para o DISCRIMINADOR: com cada cliente numa tabela
    propria, uma trava POR TABELA deixaria a vazao escalar; a global nao.
    """
    c = pp.Cliente()
    c.call({"op": "login", "usuario": "root", "senha": pp.SENHA})
    fim = time.monotonic() + segundos
    n = 0
    while time.monotonic() < fim:
        if modo.startswith("ler"):
            c.call({"op": "varrer", "database": "t", "tabela": tabela,
                    "max": 50}, exigir=False)
        elif modo == "sem-trava":
            # O CONTROLE. Mesmo soquete, mesmo JSON, mesmo despacho, mesmo
            # cliente Python -- e `ping` responde SEM tomar `travar_dados`.
            # O que ele nao escalar nao e culpa da trava: e do medidor ou da
            # maquina. A diferenca entre esta curva e a de `ler` E a trava.
            c.call({"op": "ping"}, exigir=False)
        else:
            c.call({"op": "inserir", "database": "t", "tabela": tabela,
                    "linha": {"nome": "x"}}, exigir=False)
        n += 1
    fila.put(n)


def rodada(modo, n, segundos, vigia, separadas=False):
    """N processos em paralelo pelo mesmo prazo -> operacoes por segundo.

    A ocupacao da maquina sai do `quieta.Vigia` e nao mais de uma leitura
    solta do `/proc/stat` aqui: uma amostra por rodada nao pega o vizinho que
    comeca depois dela, e nao compara as pontas da bateria. O vigia faz as
    tres coisas, e -- o que importa -- RECUSA publicar quando a maquina se
    mexeu, em vez de imprimir o numero com uma ressalva ao lado. Ressalva nao
    viaja junto do numero para o documento seguinte.
    """
    fila = mp.Queue()
    procs = [mp.Process(target=trabalhador,
                        args=(modo, segundos, fila, f"c{i}" if separadas else "c"))
             for i in range(n)]
    t0 = time.monotonic()
    for p in procs:
        p.start()
    # Os clientes mais o servidor sao NOSSOS: o vigia so acusa o excedente.
    a = vigia.durante_a_rodada(meus=n + 1)
    total = sum(fila.get() for _ in procs)
    for p in procs:
        p.join()
    return total / (time.monotonic() - t0), a.ocupada


def principal():
    if not pp.PHXSQLD.exists():
        print(f"falta {pp.PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    # Nucleos que ESTE processo pode usar, e nao os da maquina: dentro de
    # contentor com afinidade o teto real e outro, e um teto errado faz «N
    # acima dos nucleos» ser dito na hora errada.
    nucleos = quieta.nucleos()
    vigia = quieta.Vigia().abrir()
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
        # As tabelas por cliente, para o discriminador: cada uma com o mesmo
        # tanto de linha, senao a comparacao mediria o volume e nao a trava.
        for i in range(4):
            c.call({"op": "criar_tabela", "database": "t", "tabela": f"c{i}",
                    "colunas": [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                                {"nome": "nome", "tipo": "Str(20)"}],
                    "indices": [{"nome": "porId", "colunas": ["id"],
                                 "unico": True, "primario": True}]}, exigir=False)
            for _ in range(LINHAS):
                c.call({"op": "inserir", "database": "t", "tabela": f"c{i}",
                        "linha": {"nome": "semente"}}, exigir=False)

        for modo, separadas in (("sem-trava", False), ("ler", False),
                                ("gravar", False), ("ler-tabelas-separadas", True),
                                ("gravar-tabelas-separadas", True)):
            base = None
            saida[modo] = {}
            for n in (1, 2, 4):
                ops, cpu = rodada(modo, n, SEGUNDOS, vigia, separadas)
                if base is None:
                    base = ops
                    if modo == "sem-trava" and vigia.controle_antes is None:
                        vigia.controle_antes = ops
                saida[modo][n] = {"ops": ops, "ganho": ops / base, "cpu": cpu}
        # O CONTROLE de novo, no fim. Se o `ping` -- que nem toma a trava --
        # mudou entre o comeco e o fim, quem mudou foi a maquina, e a bateria
        # inteira perdeu a comparacao.
        vigia.controle_depois = rodada("sem-trava", 1, SEGUNDOS, vigia)[0]
        vigia.fechar()
        vigia.relatar()

        sujo_vale = "--mesmo-sujo" in sys.argv
        if not vigia.publicavel() and not sujo_vale:
            print("Nenhum numero sai desta rodada. Rode com a maquina parada,")
            print("ou use --mesmo-sujo para depurar o proprio arnes.")
            return 1
        if not vigia.publicavel():
            print(">>> NUMEROS SUJOS: a maquina nao estava parada. NAO CITAR. <<<\n")

        for modo, curva in saida.items():
            if not isinstance(curva, dict) or 1 not in curva:
                continue
            print(f"-- {modo}")
            for n, d in curva.items():
                teto = "  (N > nucleos: PISO, nao veredito)" if n > nucleos - 1 else ""
                print(f"   {n} cliente(s): {d['ops']:8.0f} op/s   "
                      f"ganho {d['ganho']:.2f}x   cpu {d['cpu']:3.0f}%{teto}")
            print()
        if "--json" in sys.argv:
            print(json.dumps(saida, indent=2))
        return 0 if vigia.publicavel() else 1
    finally:
        srv.parar()


if __name__ == "__main__":
    raise SystemExit(principal())
