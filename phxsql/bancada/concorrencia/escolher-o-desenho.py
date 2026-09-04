#!/usr/bin/env python3
"""Trava por tabela, `RwLock` ou MVCC: a medicao que ESCOLHE entre os tres.

    python3 bancada/concorrencia/escolher-o-desenho.py
    SEGUNDOS=5 python3 bancada/concorrencia/escolher-o-desenho.py
    python3 bancada/concorrencia/escolher-o-desenho.py --mesmo-sujo   # ver texto

Por que ele existe, e o que ele NAO e
--------------------------------------
O `a-trava-serializa.py` respondeu a premissa da SP000011: a trava global
custa. Ele nao responde a pergunta seguinte, que e a que decide o trabalho --
**qual desenho substitui a trava**. Trava por tabela, `RwLock` e MVCC compram
coisas DIFERENTES, e nenhum dos tres esta implementado aqui, entao nao ha o que
cronometrar contra o que.

O que se pode medir e o TETO de cada um: quanto de paralelismo cada desenho
teria para recuperar, se fosse perfeito. Um desenho cujo teto e 1,05x esta
respondido antes de comecar.

Os tres tetos, e o que separa um do outro
------------------------------------------
* **Trava por tabela** so ajuda quando os clientes estao em tabelas
  diferentes. O teto dela e `leitura em tabelas separadas` contra `leitura na
  mesma tabela`: se as duas curvas forem iguais, separar por tabela nao compra
  nada, porque nao e a tabela que serializa.
* **`RwLock`** so ajuda entre LEITORES. O teto dele e `leitura` contra a curva
  de CONTROLE (o `ping`, que nao toma a trava): a distancia entre as duas e o
  paralelismo que a exclusividade da leitura esta comendo.
* **MVCC** e o unico que tira o leitor de tras do ESCRITOR. O teto dele nao e
  vazao, e ESPERA -- e ele se le em DUAS contas, nao numa:
    - contra o leitor SOZINHO (`teto-do-mvcc-p99`), que e o que a espera custa
      por inteiro;
    - contra DOIS LEITORES (`teto-do-mvcc-exclusivo`), que e o que sobra
      depois de descontar o que o `RwLock` ja recupera.
  Esta linha dizia que «`RwLock` nao mexe nesse numero», e estava errada: ele
  mexe na parte que e de haver um segundo cliente qualquer. O que resta ao
  MVCC e a segunda conta, e ela deu 0,91x a 1,13x em duas baterias limpas de
  04/09 -- ruido, contra 1,19x-1,38x da primeira.

A media esconde justamente o que se procura
--------------------------------------------
Uma parada de 40 ms num `varrer` de 300 us, uma vez a cada 200 gravacoes,
some na media e aparece inteira no p99. Por isso aqui se mede DISTRIBUICAO, e
nao vazao apenas -- e por isso a bateria roda nas duas durabilidades: no
`por_lote` (o padrao) o `fsync` acontece a cada 200 gravacoes ou 200 ms; no
`por_operacao` ele acontece em TODA gravacao, com a trava global na mao. Sao
dois servidores diferentes vestidos com a mesma roupa, e o desenho certo pode
nao ser o mesmo para os dois.

A recusa e o comportamento
--------------------------
Numero de concorrencia tirado de maquina ocupada nao e numero: e ruido com
casas decimais, e ele APONTA PARA O MESMO LADO que a hipotese (a curva achata
igual). O `quieta.Vigia` mede a maquina nas duas pontas e no meio, e quando ela
nao estava parada esta bateria **nao imprime numero nenhum**. O `--mesmo-sujo`
existe para depurar o proprio arnes e carimba tudo o que sai.
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
TOKEN = "escolher-o-desenho"
SENHA = "desenho-4321"
SEGUNDOS = float(os.environ.get("SEGUNDOS", "3"))
LINHAS = int(os.environ.get("LINHAS", "2000"))
CLIENTES = [int(x) for x in os.environ.get("CLIENTES", "1,2,4").split(",")]
# O TAMANHO DA LEITURA, e o campo dela e `max` -- nao `limite`.
#
# Ate 04/09 esta bancada mandava `"limite": 50` num pedido de `varrer`. O
# `op_varrer` le `Servidor::limite(p)`, que le o campo **`max`**: `"limite"`
# nao existe no protocolo, entao TODA leitura caia no teto de configuracao e
# devolvia **1.000 linhas**. Provado com o servidor de pe: `limite: 1`,
# `limite: 50` e `limite: 200` devolvem 1000, 1000 e 1000; `max: 50` devolve
# 50. Ver `docs/CONCORRENCIA.md` §14.
#
# O padrao fica em 1.000 DE PROPOSITO: e o que as baterias de 03/09 e 04/09
# mediram de fato, e mudar o numero junto com o campo tornaria as corridas
# novas incomparaveis com as publicadas. Quem quiser o perfil de leitura curta
# manda `LINHAS_LIDAS=50`.
LINHAS_LIDAS = int(os.environ.get("LINHAS_LIDAS", "1000"))
TABELAS = 4


# --------------------------------------------------------------- o servidor


class Servidor:
    """Um `phxsqld` proprio, em porta propria, morto pelo PID.

    Proprio e nao emprestado de outra bancada porque esta precisa mexer em
    `recursos.durabilidade`, que e justamente a variavel do experimento.
    Nunca `pkill`: matar por nome derrubaria o servidor de outra frente na
    mesma maquina, e isso ja aconteceu aqui.
    """

    def __init__(self, porta, durabilidade="por_lote"):
        self.porta = porta
        self.base = Path(f"/tmp/phx-desenho-{os.getpid()}-{porta}")
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


# ------------------------------------------------------------ os perfis


def pedido(perfil, tabela, linhas=None):
    """O pedido que a bancada mede. `linhas` existe para a GUARDA poder pedir
    uma pagina de tamanho conhecido pelo mesmo caminho -- sem ela, a guarda
    teria de montar o pedido por conta propria e voltaria a conferir o servidor
    em vez de conferir esta funcao."""
    if perfil == "controle":
        # A CURVA DE CONTROLE. Mesmo soquete, mesmo JSON, mesmo despacho -- e
        # `ping` responde SEM tomar a trava de dados. O que ele nao escalar
        # nao e culpa da trava: e do medidor ou da maquina.
        return {"op": "ping"}
    if perfil == "ler":
        return {"op": "varrer", "database": "t", "tabela": tabela,
                "max": LINHAS_LIDAS if linhas is None else linhas}
    return {"op": "inserir", "database": "t", "tabela": tabela,
            "linha": {"nome": "x"}}


def trabalhador(perfil, tabela, porta, segundos, fila):
    """Um processo cliente. PROCESSO, e nao thread: com threads a GIL do
    Python limitaria a vazao e o medidor «provaria» a serializacao do servidor
    medindo a de si mesmo."""
    c = Cliente(porta)
    c.call({"op": "login", "usuario": "root", "senha": SENHA})
    p = pedido(perfil, tabela)
    fim = time.monotonic() + segundos
    esperas = []
    while time.monotonic() < fim:
        t0 = time.perf_counter_ns()
        c.call(dict(p), exigir=False)
        esperas.append(time.perf_counter_ns() - t0)
    esperas.sort()
    n = len(esperas)
    quantil = lambda q: esperas[min(n - 1, int(q * n))] / 1000.0 if n else 0.0
    fila.put({"perfil": perfil, "ops": n,
              "p50": quantil(0.50), "p95": quantil(0.95),
              "p99": quantil(0.99), "pior": esperas[-1] / 1000.0 if n else 0.0})


def rodada(perfis, porta, segundos, vigia, separadas=False):
    """Roda os perfis em paralelo pelo mesmo prazo.

    `perfis` e uma LISTA e nao (modo, n) porque a rodada que mais importa e
    heterogenea: N leitores com UM escritor ao lado. E ela que separa o que o
    `RwLock` compra do que o MVCC compra.
    """
    fila = mp.Queue()
    procs = []
    for i, perfil in enumerate(perfis):
        tabela = f"c{i % TABELAS}" if separadas else "c"
        procs.append(mp.Process(target=trabalhador,
                                args=(perfil, tabela, porta, segundos, fila)))
    t0 = time.monotonic()
    for p in procs:
        p.start()
    # Os clientes mais o servidor sao NOSSOS: o vigia so acusa o excedente.
    vigia.durante_a_rodada(meus=len(perfis) + 1)
    saidas = [fila.get() for _ in procs]
    for p in procs:
        p.join()
    decorrido = time.monotonic() - t0
    por_perfil = {}
    for s in saidas:
        d = por_perfil.setdefault(s["perfil"], {"ops": 0, "p50": [], "p95": [],
                                                "p99": [], "pior": 0.0})
        d["ops"] += s["ops"]
        for q in ("p50", "p95", "p99"):
            d[q].append(s[q])
        d["pior"] = max(d["pior"], s["pior"])
    for d in por_perfil.values():
        d["ops_s"] = d.pop("ops") / decorrido
        for q in ("p50", "p95", "p99"):
            # A MEDIANA dos quantis dos clientes, e nao a media: um cliente que
            # pegou o `fsync` inteiro nao deve mover o numero dos outros.
            v = sorted(d[q])
            d[q] = v[len(v) // 2]
    return por_perfil


# ------------------------------------------------------------- a bateria


def semear(c):
    colunas = [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
               {"nome": "nome", "tipo": "Str(20)"}]
    indices = [{"nome": "porId", "colunas": ["id"], "unico": True,
                "primario": True}]
    c.call({"op": "criar_database", "database": "t"}, exigir=False)
    for nome in ["c"] + [f"c{i}" for i in range(TABELAS)]:
        c.call({"op": "criar_tabela", "database": "t", "tabela": nome,
                "colunas": colunas, "indices": indices}, exigir=False)
        # Cada tabela com o MESMO tanto de linha: com volumes diferentes, a
        # comparacao «mesma tabela x separadas» mediria o volume e nao a trava.
        for _ in range(LINHAS):
            c.call({"op": "inserir", "database": "t", "tabela": nome,
                    "linha": {"nome": "semente"}}, exigir=False)


def bateria(durabilidade, vigia):
    porta = quieta.porta_livre()
    srv = Servidor(porta, durabilidade)
    try:
        c = Cliente(porta)
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        semear(c)
        # A GUARDA, antes de qualquer numero: voltou o que se pediu?
        quieta.confira_a_pagina(c.call, lambda n: pedido("ler", "c", n))

        saida = {"durabilidade": durabilidade, "porta": porta, "curvas": {},
                 "espera": {}}

        # As curvas de vazao, e o controle junto -- medido na MESMA bateria e
        # nao noutro dia, senao a comparacao atravessa duas maquinas.
        for nome, perfil, separadas in (
                ("controle", "controle", False),
                ("leitura-mesma-tabela", "ler", False),
                ("leitura-tabelas-separadas", "ler", True),
                ("escrita-mesma-tabela", "gravar", False),
                ("escrita-tabelas-separadas", "gravar", True)):
            curva = {}
            for n in CLIENTES:
                r = rodada([perfil] * n, porta, SEGUNDOS, vigia, separadas)
                curva[n] = r[perfil]
                if nome == "controle" and n == 1:
                    if vigia.controle_antes is None:
                        vigia.controle_antes = r[perfil]["ops_s"]
            saida["curvas"][nome] = curva

        # A ESPERA: o mesmo leitor, sozinho e com um escritor ao lado. E o
        # unico par que separa o teto do `RwLock` do teto do MVCC.
        sozinho = rodada(["ler"], porta, SEGUNDOS, vigia)["ler"]
        com_escritor = rodada(["ler", "gravar"], porta, SEGUNDOS, vigia)["ler"]
        dois_leitores = rodada(["ler", "ler"], porta, SEGUNDOS, vigia)["ler"]
        saida["espera"] = {"leitor-sozinho": sozinho,
                           "dois-leitores": dois_leitores,
                           "leitor-com-escritor": com_escritor}

        # O controle DE NOVO, no fim: se ele se moveu, quem mudou foi a
        # maquina, e a bateria inteira perde a comparacao.
        vigia.controle_depois = rodada(["controle"], porta, SEGUNDOS,
                                       vigia)["controle"]["ops_s"]
        return saida
    finally:
        srv.parar()


# ------------------------------------------------------------- o relatorio


def tetos(b):
    """O que cada desenho teria para recuperar, do que foi medido."""
    c = b["curvas"]
    t = {}
    n = max(CLIENTES)
    if n in c["controle"] and c["controle"][1]["ops_s"]:
        ganho = lambda nome: (c[nome][n]["ops_s"] / c[nome][1]["ops_s"]
                              if c[nome][1]["ops_s"] else 0.0)
        t["controle"] = ganho("controle")
        t["leitura"] = ganho("leitura-mesma-tabela")
        t["leitura-separadas"] = ganho("leitura-tabelas-separadas")
        t["escrita"] = ganho("escrita-mesma-tabela")
        t["escrita-separadas"] = ganho("escrita-tabelas-separadas")
        t["teto-do-rwlock"] = (t["controle"] / t["leitura"]
                               if t["leitura"] else 0.0)
        t["teto-da-trava-por-tabela"] = (t["leitura-separadas"] / t["leitura"]
                                         if t["leitura"] else 0.0)
    e = b["espera"]
    if e["leitor-sozinho"]["p99"]:
        t["teto-do-mvcc-p99"] = (e["leitor-com-escritor"]["p99"]
                                 / e["leitor-sozinho"]["p99"])
        t["teto-do-rwlock-p99"] = (e["dois-leitores"]["p99"]
                                   / e["leitor-sozinho"]["p99"])
        # O TETO EXCLUSIVO do MVCC, e ele nao e o de cima.
        #
        # `leitor-com-escritor / leitor-sozinho` mede DUAS coisas somadas: o
        # custo de haver qualquer segundo cliente -- que o `RwLock` ja
        # recupera -- e o custo de esse cliente ser um ESCRITOR, que e a unica
        # parte que so o MVCC endereca. Creditar a soma ao MVCC e credita-lo
        # pelo trabalho do `RwLock`.
        #
        # Medido em 04/09 (duas baterias limpas): o teto do par deu 1,19x a
        # 1,38x e o exclusivo deu 0,91x a 1,13x -- uma corrida com o escritor
        # ao lado MAIS BARATO que outro leitor. Sem esta linha, quem le o
        # relatorio conclui 1,30x, que foi o que eu conclui.
        if e["dois-leitores"]["p99"]:
            t["teto-do-mvcc-exclusivo"] = (e["leitor-com-escritor"]["p99"]
                                           / e["dois-leitores"]["p99"])
    return t


def imprimir(b, t):
    n = max(CLIENTES)
    print(f"== durabilidade: {b['durabilidade']} ==\n")
    print(f"-- vazao, ganho de {n} clientes sobre 1")
    for nome, curva in b["curvas"].items():
        base = curva[1]["ops_s"]
        linha = "   ".join(
            f"{k}:{curva[k]['ops_s'] / base:.2f}x" if base else f"{k}:?"
            for k in CLIENTES)
        print(f"   {nome:<28} {linha}")
    print()
    print("-- espera de UM leitor (us), pelo p99")
    for nome, d in b["espera"].items():
        print(f"   {nome:<24} p50 {d['p50']:>9.1f}  p95 {d['p95']:>9.1f}  "
              f"p99 {d['p99']:>9.1f}  pior {d['pior']:>10.1f}")
    print()
    print("-- os tetos: quanto cada desenho teria para recuperar")
    print(f"   trava por tabela   {t.get('teto-da-trava-por-tabela', 0):.2f}x"
          "   (leitura em tabelas separadas contra a mesma tabela)")
    print(f"   RwLock             {t.get('teto-do-rwlock', 0):.2f}x"
          "   (o controle, que nao toma a trava, contra a leitura)")
    print(f"   RwLock, na espera  {t.get('teto-do-rwlock-p99', 0):.2f}x"
          "   (p99 de dois leitores contra um)")
    print(f"   MVCC, na espera    {t.get('teto-do-mvcc-p99', 0):.2f}x"
          "   (p99 do leitor COM escritor contra SOZINHO)")
    print(f"   MVCC, EXCLUSIVO    {t.get('teto-do-mvcc-exclusivo', 0):.2f}x"
          "   (contra DOIS LEITORES: o que o RwLock nao recupera)")
    print()


def autoteste():
    """A conta do teto EXCLUSIVO, provada contra as duas baterias limpas.

    Prova real nos dois sentidos: com a formula certa os quatro numeros batem
    as corridas guardadas em `corridas/`, e com a formula ANTIGA (dividir pelo
    leitor sozinho) nenhum deles bate. Se alguem trocar o denominador de volta,
    isto falha aqui em vez de falhar tres documentos adiante.
    """
    # As quatro medicoes de 04/09, das corridas guardadas: (sozinho, dois
    # leitores, com escritor, exclusivo esperado).
    casos = [
        ("A por_lote", 6583.2, 8546.0, 8586.3, 1.00),
        ("B por_lote", 7316.5, 9579.6, 8703.6, 0.91),
        ("A por_operacao", 6905.6, 8406.5, 9525.4, 1.13),
        ("B por_operacao", 7071.4, 8637.9, 8780.0, 1.02),
    ]
    ok = True
    for nome, soz, dois, esc, esperado in casos:
        # `tetos` calcula a vazao antes da espera, e precisa das curvas: um
        # dicionario vazio nao exercita o mesmo caminho que uma bateria de
        # verdade percorre, e este autoteste existe para exercitar esse.
        curva = {k: {"ops_s": 1000.0} for k in CLIENTES}
        b = {"curvas": {n: dict(curva) for n in (
                 "controle", "leitura-mesma-tabela",
                 "leitura-tabelas-separadas", "escrita-mesma-tabela",
                 "escrita-tabelas-separadas")},
             "espera": {
            "leitor-sozinho": {"p99": soz},
            "dois-leitores": {"p99": dois},
            "leitor-com-escritor": {"p99": esc}}}
        t = tetos(b)
        deu = t["teto-do-mvcc-exclusivo"]
        bate = abs(deu - esperado) < 0.005
        ok &= bate
        # E o denominador ERRADO (o leitor sozinho) tem de dar OUTRA coisa:
        # sem esta metade, trocar o denominador passaria despercebido.
        antigo = t["teto-do-mvcc-p99"]
        distingue = abs(antigo - esperado) >= 0.05
        ok &= distingue
        print(f"   {'ok ' if bate and distingue else 'FALHOU'} {nome:16} "
              f"exclusivo {deu:.2f}x (esperado {esperado:.2f}x)  "
              f"| pelo denominador antigo daria {antigo:.2f}x")
    print("   autoteste:", "passou" if ok else "FALHOU")
    return 0 if ok else 1


def principal():
    if "--autoteste" in sys.argv:
        print("=== autoteste: a conta do teto exclusivo ===")
        return autoteste()
    if not PHXSQLD.exists():
        print(f"falta {PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    sujo_vale = "--mesmo-sujo" in sys.argv
    print("=== trava por tabela, RwLock ou MVCC: o teto de cada um ===")
    print(f"    {quieta.nucleos()} nucleos ao alcance deste processo | "
          f"{SEGUNDOS:.0f}s por rodada | {LINHAS} linhas semeadas")
    print(f"    clientes: {CLIENTES} | faixa de portas {quieta.FAIXA}\n")

    vigia = quieta.Vigia().abrir()
    baterias = [bateria(d, vigia) for d in ("por_lote", "por_operacao")]
    vigia.fechar()
    vigia.relatar()

    if not vigia.publicavel() and not sujo_vale:
        print("Nenhum numero sai desta rodada. Rode de novo com a maquina")
        print("parada, ou use --mesmo-sujo para depurar o proprio arnes.")
        return 1
    if not vigia.publicavel():
        print(">>> NUMEROS SUJOS: a maquina nao estava parada. NAO CITAR. <<<\n")

    tudo = {"sujo": not vigia.publicavel(), "baterias": []}
    for b in baterias:
        t = tetos(b)
        imprimir(b, t)
        tudo["baterias"].append({"medido": b, "tetos": t})
    if "--json" in sys.argv:
        print(json.dumps(tudo, indent=2))
    return 0 if vigia.publicavel() else 1


if __name__ == "__main__":
    raise SystemExit(principal())
