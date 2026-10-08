#!/usr/bin/env python3
"""Pedido 678: 20 caixas e 1 central, derrubando o central no meio do expediente.

    python3 bancada/caixa-offline/medir.py [voltas] [--caixas N] [--fase S]

Desenho ESPELHO (pedido 325, `docs/propostas/caixa-offline-325.md`, MANUAL
§16.1): cada caixa e um `phxsqld` dono do database `caixaNN`; o central e
replica das N origens com `"espelho": true`; o cadastro (`cadastro.produtos`)
so se escreve no central e desce para os caixas pela via inversa, tambem com
`"espelho": true`. Os dois papeis sao `replica` com escrita local -- cada um
escreve no que e dele e recebe o que e do outro.

O que cada volta mede, e por que cada numero:

1. **Chegada simultanea das N origens no central.** Para cada venda, o tempo
   do `COMMIT` no caixa ate ela aparecer no central (min/mediana/p95/max). E
   a sonda da trava global: `ping` (nao toca a trava) e `varrer` (toca) no
   central, a cada 20 ms. So o `varrer` subir e o que aponta a trava.
2. **O central DERRUBADO** (`SIGKILL`, pelo PID do `Popen`): os caixas seguem
   vendendo. Conta-se a venda recusada e a latencia da venda no caixa ANTES,
   DURANTE e DEPOIS da queda.
3. **O central religado:** tempo ate atender e ate alcancar as N origens (o
   que os caixas tinham cometido no instante do religar). E a conferencia de
   que nenhuma venda chegou pela metade (sanduiche de contagens no central
   durante todo o alcance, contra a soma dos itens que o caixa cometeu) nem
   duplicada (contagem e SHA-256 linha a linha das tres tabelas, caixa contra
   central, no fim).
4. **O diario do caixa por venda:** bytes de `.log` do `caixaNN` cometidos
   por venda.

Nada aqui usa `pkill`: os servidores sao os `Popen` desta bancada, e morrem
pelo PID guardado; a rede de seguranca do fim so mata `phxsqld` cujo `cwd`
esta DENTRO do diretorio desta bancada.
"""
import hashlib
import json
import os
import random
import shutil
import signal
import socket
import statistics
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = os.environ.get("PHX_CAIXA_OFFLINE", "/tmp/phx-caixa-offline")

PORTA_CENTRAL = 7300
TOKEN = "caixa-offline"
# Um segundo e o minimo que o `config.json` aceita (`reconectar_em` vira
# `max(1)`). E ele que domina a latencia de chegada: a replica que perguntou e
# nao achou nada DORME isso antes de perguntar de novo. Fica gravado no
# resultado para ninguem ler o numero como custo de transporte.
RECONECTAR_EM = 1
PRODUTOS = 50
ESTOQUE_INICIAL = 1_000_000
# Pausa entre vendas de um caixa: 3 a 10 itens a cada 0,1-0,3 s e um caixa
# muito mais rapido que um de verdade, de proposito -- comprime o expediente.
PAUSA = (0.10, 0.30)
# O disco desta maquina tinha ~2,3 GB livres; a bancada para se o livre cair
# abaixo disto, antes de encher o disco de alguem.
PISO_LIVRE = 1_300_000_000


def livre():
    st = os.statvfs(os.path.dirname(BASE.rstrip("/")) or "/")
    return st.f_bavail * st.f_frsize


class Ligacao:
    """Uma conexao que FICA -- a transacao vive na sessao."""

    def __init__(self, porta, prazo=10.0):
        s = socket.create_connection(("127.0.0.1", porta), timeout=prazo)
        # Sem isto cada pedido pequeno paga Nagle com ACK atrasado (~40 ms).
        s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.s = s
        self.f = s.makefile("rwb")

    def fala(self, p):
        p = dict(p)
        p["token"] = TOKEN
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise ConnectionError("conexao fechada")
        return json.loads(linha.decode())

    def fechar(self):
        # `makefile` segura o descritor: fechar so o soquete deixa o fd vivo
        # e o servidor nunca ve o fim da conexao (licao do BULKINSERT).
        for x in (self.f, self.s):
            try:
                x.close()
            except OSError:
                pass


def exigir(lig, p):
    r = lig.fala(p)
    if not r.get("ok"):
        raise RuntimeError(f"{p.get('op')}: {r}")
    return r.get("resultado")


def esperar_porta(porta, segundos=30):
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < segundos:
        try:
            lig = Ligacao(porta, prazo=1)
            r = lig.fala({"op": "ping"})
            lig.fechar()
            if r.get("ok"):
                return time.perf_counter() - t0
        except OSError:
            pass
        time.sleep(0.02)
    raise SystemExit(f"porta {porta} nao atendeu em {segundos}s")


def porta_livre(porta):
    s = socket.socket()
    # O `TcpListener` da std liga SO_REUSEADDR; sem ele aqui, o TIME_WAIT da
    # volta anterior pareceria uma bancada vizinha no ar.
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        s.bind(("127.0.0.1", porta))
        return True
    except OSError:
        return False
    finally:
        s.close()


# ---------------------------------------------------------------- montagem

def origem(nome, porta, databases):
    return {"nome": nome, "host": "127.0.0.1", "porta": porta, "token": TOKEN,
            "databases": databases, "reconectar_em": RECONECTAR_EM,
            # bancada de loopback: a cifra do fio tem bancada propria
            # (`bancada/cifra-do-fio/`), e aqui o aperto so somaria ruido
            "cifra": False, "espelho": True}


def config(porta, id_servidor, origens):
    return {
        "base": "base",
        # bancada de teste, NAO cliente do produto -- fala em claro para medir
        # a replicacao espelho sem o aperto de mao no meio
        "bind": f"127.0.0.1:{porta}", "cifra_fio": {"exigir": False},
        "token": TOKEN,
        "web": {"ligado": False},
        "replicacao": {"papel": "replica", "id_servidor": id_servidor,
                       "imagem_da_linha": True, "origens": origens},
    }


class Bancada:
    def __init__(self, n_caixas):
        self.n = n_caixas
        self.portas = {f"caixa{i:02d}": PORTA_CENTRAL + i
                       for i in range(1, n_caixas + 1)}
        self.pids = {}

    def dir(self, nome):
        return os.path.join(BASE, nome)

    def escrever(self):
        os.makedirs(self.dir("central"), exist_ok=True)
        origens = [origem(c, p, [c]) for c, p in self.portas.items()]
        with open(os.path.join(self.dir("central"), "config.json"), "w") as f:
            json.dump(config(PORTA_CENTRAL, "central", origens), f, indent=2)
        for c, p in self.portas.items():
            os.makedirs(self.dir(c), exist_ok=True)
            with open(os.path.join(self.dir(c), "config.json"), "w") as f:
                json.dump(config(p, c, [origem("central", PORTA_CENTRAL,
                                               ["cadastro"])]), f, indent=2)

    def subir(self, nome):
        d = self.dir(nome)
        log = open(os.path.join(d, "servidor.log"), "a")
        # start_new_session no lugar de `setsid`: o PID do Popen e o do
        # proprio phxsqld, e e por ele que a queda acontece.
        p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log,
                             stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                             start_new_session=True)
        log.close()
        self.pids[nome] = p

    def matar(self, nome, sinal):
        p = self.pids.pop(nome, None)
        if p is None:
            return
        try:
            os.kill(p.pid, sinal)
        except ProcessLookupError:
            pass
        try:
            p.wait(timeout=30)
        except subprocess.TimeoutExpired:
            os.kill(p.pid, signal.SIGKILL)
            p.wait()

    def derrubar_tudo(self):
        for nome in list(self.pids):
            self.matar(nome, signal.SIGTERM)
        # Rede de seguranca por caminho real: so `phxsqld` com cwd DENTRO
        # desta bancada (um Popen que escapou de uma volta abortada).
        raiz = os.path.realpath(BASE)
        for pid in os.listdir("/proc"):
            if not pid.isdigit():
                continue
            try:
                if os.path.basename(os.path.realpath(f"/proc/{pid}/exe")) != "phxsqld":
                    continue
                cwd = os.readlink(f"/proc/{pid}/cwd").replace(" (deleted)", "")
                if os.path.realpath(cwd).startswith(raiz + os.sep):
                    os.kill(int(pid), signal.SIGKILL)
            except OSError:
                continue


def criar_cadastro(lig):
    exigir(lig, {"op": "criar_database", "database": "cadastro"})
    exigir(lig, {"op": "criar_tabela", "database": "cadastro", "tabela": "produtos",
                 "motivo_obrigatorio": False,
                 "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                             {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                             {"nome": "preco", "tipo": "Decimal(12,2)"}],
                 "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                              "primario": True}]})
    exigir(lig, {"op": "inserir_lote", "database": "cadastro", "tabela": "produtos",
                 "linhas": [{"id": k, "nome": f"Produto {k:03d}",
                             "preco": f"{1 + (k * 37) % 90}.{k % 100:02d}"}
                            for k in range(1, PRODUTOS + 1)]})


def criar_caixa(lig, db):
    exigir(lig, {"op": "criar_database", "database": db})
    exigir(lig, {"op": "criar_tabela", "database": db, "tabela": "vendas",
                 "motivo_obrigatorio": False,
                 "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                             {"nome": "itens", "tipo": "Int4"},
                             {"nome": "total", "tipo": "Decimal(12,2)"},
                             {"nome": "momento_ms", "tipo": "Int8"}],
                 "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                              "primario": True}]})
    # A chave nasce conferida (pétrea), e por isso o indice dos dois lados.
    exigir(lig, {"op": "criar_tabela", "database": db, "tabela": "itens",
                 "motivo_obrigatorio": False,
                 "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                             {"nome": "venda", "tipo": "Int8", "obrigatoria": True},
                             {"nome": "produto", "tipo": "Int4"},
                             {"nome": "qtd", "tipo": "Int4"},
                             {"nome": "preco", "tipo": "Decimal(12,2)"}],
                 "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                              "primario": True},
                             {"nome": "porVenda", "colunas": ["venda"]}],
                 "chaves_estrangeiras": [{"nome": "fk_venda", "colunas": ["venda"],
                                          "tabela_ref": "vendas",
                                          "colunas_ref": ["id"]}]})
    exigir(lig, {"op": "criar_tabela", "database": db, "tabela": "estoque",
                 "motivo_obrigatorio": False,
                 "colunas": [{"nome": "produto", "tipo": "Int4", "obrigatoria": True},
                             {"nome": "qtd", "tipo": "Int8"}],
                 "indices": [{"nome": "porProduto", "colunas": ["produto"],
                              "unico": True, "primario": True}]})
    exigir(lig, {"op": "inserir_lote", "database": db, "tabela": "estoque",
                 "linhas": [{"produto": k, "qtd": ESTOQUE_INICIAL}
                            for k in range(1, PRODUTOS + 1)]})


def varrer_tudo(lig, db, tabela):
    linhas, depois = [], 0
    while True:
        d = exigir(lig, {"op": "varrer", "database": db, "tabela": tabela,
                         "max": 1000, "depois": depois, "visao": "todas"})
        linhas.extend(d["linhas"])
        if not d.get("ha_mais") or not d["linhas"]:
            return linhas
        depois = d["cursor_fim"]


def retrato(lig, db, tabela):
    """SHA-256 de cada linha inteira, com o rowid -- contar nao acha a linha
    que atravessou errada."""
    h = hashlib.sha256()
    ls = varrer_tudo(lig, db, tabela)
    for l in ls:
        h.update(json.dumps(l, sort_keys=True, ensure_ascii=False).encode())
    return len(ls), h.hexdigest()[:16]


def contar(lig, db, tabela):
    r = lig.fala({"op": "agrupar", "database": db, "tabela": tabela,
                  "agregados": [{"funcao": "contagem", "apelido": "n"}]})
    if not r.get("ok"):
        return None
    gs = r["resultado"].get("linhas") or r["resultado"].get("grupos") or []
    return int(gs[0]["n"]) if gs else 0


def bytes_do_diario(dir_db):
    tot_log, tot = 0, 0
    for raiz, _, arqs in os.walk(dir_db):
        for a in arqs:
            t = os.path.getsize(os.path.join(raiz, a))
            tot += t
            if a.endswith(".log"):
                tot_log += t
    return tot_log, tot


# ---------------------------------------------------------------- a volta

class Estado:
    def __init__(self, caixas):
        self.fase = "montagem"
        self.parar_vendas = threading.Event()
        self.parar_obs = threading.Event()
        self.trava = threading.Lock()
        # por caixa: id -> (itens, wall do COMMIT ok, fase do expediente)
        self.cometidas = {c: {} for c in caixas}
        self.itens_por_id = {c: {} for c in caixas}
        self.lat_venda = {}            # fase -> [ms]
        self.recusadas = []            # (caixa, fase, motivo)
        self.chegada = {c: {} for c in caixas}   # id -> wall de chegada
        self.sonda = {}                # fase -> {"ping": [], "varrer": []}
        self.meias = []                # (caixa, vendas, itens, esperado)
        self.sanduiches = 0
        self.rodadas_poller = []


def vendedor(est, caixa, porta, semente):
    rng = random.Random(semente)
    lig = Ligacao(porta)
    db = caixa
    estoque = {}
    for l in varrer_tudo(lig, db, "estoque"):
        estoque[l["produto"]] = (l["rowid"], l["qtd"])
    proximo_item = 1
    venda = 0
    while not est.parar_vendas.is_set():
        # «vende com o ultimo preco recebido»: o preco sai do cadastro LOCAL,
        # o que desceu do central -- com o central caido, o que ficou.
        precos = {l["id"]: l["preco"]
                  for l in exigir(lig, {"op": "varrer", "database": "cadastro",
                                        "tabela": "produtos", "max": 200})["linhas"]}
        venda += 1
        k = rng.randint(3, 10)
        escolha = rng.sample(sorted(precos), k)
        fase = est.fase
        with est.trava:
            est.itens_por_id[caixa][venda] = k
        t0 = time.perf_counter()
        try:
            exigir(lig, {"op": "begin", "database": db})
            total = 0.0
            itens = []
            for p in escolha:
                q = rng.randint(1, 3)
                total += q * float(precos[p])
                itens.append((p, q))
            exigir(lig, {"op": "inserir", "database": db, "tabela": "vendas",
                         "linha": {"id": venda, "itens": k, "total": f"{total:.2f}",
                                   "momento_ms": int(time.time() * 1000)}})
            for p, q in itens:
                exigir(lig, {"op": "inserir", "database": db, "tabela": "itens",
                             "linha": {"id": proximo_item, "venda": venda,
                                       "produto": p, "qtd": q, "preco": precos[p]}})
                proximo_item += 1
                rowid, saldo = estoque[p]
                exigir(lig, {"op": "atualizar", "database": db, "tabela": "estoque",
                             "rowid": rowid, "linha": {"produto": p, "qtd": saldo - q}})
                estoque[p] = (rowid, saldo - q)
            exigir(lig, {"op": "commit"})
        except (RuntimeError, OSError, ValueError) as e:
            with est.trava:
                est.recusadas.append((caixa, fase, str(e)[:300]))
                del est.itens_por_id[caixa][venda]
            try:
                lig.fala({"op": "rollback"})
            except OSError:
                lig = Ligacao(porta)
            venda -= 1
            # o estoque local da bancada pode ter andado: relido do caixa
            estoque = {l["produto"]: (l["rowid"], l["qtd"])
                       for l in varrer_tudo(lig, db, "estoque")}
            time.sleep(0.2)
            continue
        ms = (time.perf_counter() - t0) * 1000
        agora = time.time()
        with est.trava:
            est.cometidas[caixa][venda] = (k, agora, fase)
            est.lat_venda.setdefault(fase, []).append(ms)
        time.sleep(rng.uniform(*PAUSA))
    lig.fechar()


def com_central(est, corpo):
    """Laco de observador no central: religa quando ele cai."""
    lig = None
    while not est.parar_obs.is_set():
        try:
            if lig is None:
                lig = Ligacao(PORTA_CENTRAL, prazo=5)
            corpo(lig)
        except (OSError, ValueError, KeyError, ConnectionError):
            if lig is not None:
                lig.fechar()
            lig = None
            time.sleep(0.05)
    if lig is not None:
        lig.fechar()


def observador_chegada(est, caixas):
    cursor = {c: 0 for c in caixas}

    def rodada(lig):
        t0 = time.perf_counter()
        for c in caixas:
            r = lig.fala({"op": "varrer", "database": c, "tabela": "vendas",
                          "max": 1000, "depois": cursor[c], "visao": "todas"})
            if not r.get("ok"):
                continue  # o database ainda nao chegou ao central
            d = r["resultado"]
            agora = time.time()
            for l in d["linhas"]:
                est.chegada[c].setdefault(l["id"], agora)
            if d["linhas"]:
                cursor[c] = d["cursor_fim"]
        est.rodadas_poller.append((time.perf_counter() - t0) * 1000)
        time.sleep(0.01)

    com_central(est, rodada)


def observador_trava(est):
    def rodada(lig):
        t0 = time.perf_counter()
        lig.fala({"op": "ping"})
        t1 = time.perf_counter()
        lig.fala({"op": "varrer", "database": "cadastro", "tabela": "produtos",
                  "max": 1})
        t2 = time.perf_counter()
        s = est.sonda.setdefault(est.fase, {"ping": [], "varrer": []})
        s["ping"].append((t1 - t0) * 1000)
        s["varrer"].append((t2 - t1) * 1000)
        time.sleep(0.02)

    com_central(est, rodada)


def observador_meia_venda(est, caixas):
    """O sanduiche do `tests/venda-inteira-na-replica.rs`: `vendas`, `itens`,
    `vendas` de novo. So vale o retrato em que `vendas` nao mudou; nele, os
    itens no central TEM de ser a soma dos itens das vendas 1..v que o caixa
    cometeu -- nem um a mais (venda sem mae), nem um a menos (meia venda)."""
    def rodada(lig):
        for c in caixas:
            v1 = contar(lig, c, "vendas")
            if v1 is None:
                continue
            it = contar(lig, c, "itens")
            v2 = contar(lig, c, "vendas")
            if it is None or v1 != v2:
                continue
            with est.trava:
                mapa = dict(est.itens_por_id[c])
            esperado = sum(mapa.get(i, 0) for i in range(1, v1 + 1))
            est.sanduiches += 1
            if it != esperado:
                est.meias.append((c, v1, it, esperado, est.fase))
        time.sleep(0.05)

    com_central(est, rodada)


def faixa(xs, nd=1):
    if not xs:
        return None
    s = sorted(xs)

    def p(q):
        return round(s[min(len(s) - 1, int(q * len(s)))], nd)
    return {"n": len(s), "min": round(s[0], nd), "mediana": round(statistics.median(s), nd),
            "p95": p(0.95), "max": round(s[-1], nd)}


def volta(n_caixas, seg_fase, semente):
    b = Bancada(n_caixas)
    caixas = list(b.portas)
    for p in [PORTA_CENTRAL] + list(b.portas.values()):
        if not porta_livre(p):
            raise SystemExit(f"porta {p} ocupada -- outra bancada no ar?")
    if os.path.exists(BASE):
        shutil.rmtree(BASE)
    b.escrever()
    r = {"caixas": n_caixas, "segundos_por_fase": seg_fase}
    try:
        b.subir("central")
        esperar_porta(PORTA_CENTRAL)
        lc = Ligacao(PORTA_CENTRAL)
        criar_cadastro(lc)
        for c in caixas:
            b.subir(c)
        for c, p in b.portas.items():
            esperar_porta(p)
            lig = Ligacao(p)
            criar_caixa(lig, c)
            lig.fechar()
        # O cadastro tem de ter DESCIDO a todo caixa, e o esquema de todo
        # caixa SUBIDO ao central, antes de a venda comecar.
        t0 = time.perf_counter()
        for c, p in b.portas.items():
            lig = Ligacao(p)
            while (contar(lig, "cadastro", "produtos") or 0) < PRODUTOS:
                if time.perf_counter() - t0 > 120:
                    raise SystemExit(f"o cadastro nao desceu a {c} em 120 s")
                time.sleep(0.1)
            lig.fechar()
        for c in caixas:
            while (contar(lc, c, "estoque") or 0) < PRODUTOS:
                if time.perf_counter() - t0 > 120:
                    raise SystemExit(f"{c} nao subiu ao central em 120 s")
                time.sleep(0.1)
        lc.fechar()
        r["montagem_s"] = round(time.perf_counter() - t0, 2)

        diario0 = {c: bytes_do_diario(os.path.join(b.dir(c), "base", c)) for c in caixas}

        est = Estado(caixas)
        obs = [threading.Thread(target=observador_chegada, args=(est, caixas)),
               threading.Thread(target=observador_trava, args=(est,)),
               threading.Thread(target=observador_meia_venda, args=(est, caixas))]
        vend = [threading.Thread(target=vendedor,
                                 args=(est, c, b.portas[c], semente * 100 + i))
                for i, c in enumerate(caixas)]
        # Tres segundos de central OCIOSO antes da primeira venda: e o
        # contraste da sonda da trava, sem ele «o varrer subiu» nao tem
        # contra o que subir.
        est.fase = "ocioso"
        for t in obs:
            t.start()
        time.sleep(3)
        est.fase = "antes"
        for t in vend:
            t.start()
        time.sleep(seg_fase)

        # ---- a queda: SIGKILL, sem aviso, no meio do expediente
        est.fase = "durante"
        t_morte = time.time()
        b.matar("central", signal.SIGKILL)
        r["central_morto_em"] = time.strftime("%H:%M:%S")
        time.sleep(seg_fase)
        if livre() < PISO_LIVRE:
            raise SystemExit("disco abaixo do piso no meio da volta")

        # ---- a volta do central
        with est.trava:
            alvo = {c: max(est.cometidas[c], default=0) for c in caixas}
            alvo_vendas = sum(len(v) for v in est.cometidas.values())
        r["vendas_no_religar"] = alvo_vendas
        est.fase = "depois"
        t_religar = time.perf_counter()
        b.subir("central")
        r["central_atende_s"] = round(esperar_porta(PORTA_CENTRAL, 60), 3)
        alcance = None
        while time.perf_counter() - t_religar < 180:
            if all(max(est.chegada[c], default=0) >= alvo[c] for c in caixas):
                alcance = time.perf_counter() - t_religar
                break
            time.sleep(0.02)
        r["alcance_s"] = None if alcance is None else round(alcance, 2)
        resto = seg_fase - (time.perf_counter() - t_religar)
        if resto > 0:
            time.sleep(resto)
        est.parar_vendas.set()
        for t in vend:
            t.join()

        # ---- convergencia final e a conferencia
        with est.trava:
            finais = {c: max(est.cometidas[c], default=0) for c in caixas}
        t_fim = time.perf_counter()
        while time.perf_counter() - t_fim < 120:
            if all(max(est.chegada[c], default=0) >= finais[c] for c in caixas):
                break
            time.sleep(0.05)
        r["convergiu_depois_da_ultima_venda_s"] = round(time.perf_counter() - t_fim, 2)
        time.sleep(0.3)
        est.parar_obs.set()
        for t in obs:
            t.join()

        lc = Ligacao(PORTA_CENTRAL)
        conf = {}
        for c, p in b.portas.items():
            lx = Ligacao(p)
            linha = {}
            for tab in ("vendas", "itens", "estoque"):
                linha[tab] = {"caixa": retrato(lx, c, tab), "central": retrato(lc, c, tab)}
            lx.fechar()
            conf[c] = linha
        estado = lc.fala({"op": "replicacao_estado"})
        lc.fechar()
        iguais = all(v["caixa"] == v["central"] for l in conf.values() for v in l.values())
        dup = sum(1 for c in caixas
                  if conf[c]["vendas"]["central"][0] != len(est.cometidas[c]))
        r["conferencia"] = {
            "tres_tabelas_iguais_caixa_e_central": iguais,
            "caixas_com_contagem_de_vendas_diferente": dup,
            "vendas_cometidas": sum(len(v) for v in est.cometidas.values()),
            "vendas_no_central": sum(conf[c]["vendas"]["central"][0] for c in caixas),
            "itens_no_central": sum(conf[c]["itens"]["central"][0] for c in caixas),
            "sanduiches_conferidos": est.sanduiches,
            "meias_vendas_vistas": len(est.meias),
            "meias_exemplos": est.meias[:5],
            "divergentes": [c for c in caixas
                            if any(v["caixa"] != v["central"] for v in conf[c].values())],
        }
        r["replicacao_estado_central"] = (estado.get("resultado") if estado.get("ok")
                                          else estado)
        origs = (r["replicacao_estado_central"] or {}).get("origens", {})
        r["aplicados_pelo_central"] = sum(o.get("aplicados", 0) for o in origs.values())
        r["central_erros_por_origem"] = {n: o.get("ultimo_erro") for n, o in origs.items()
                                         if o.get("ultimo_erro") or o.get("parada")
                                         or o.get("transacoes_em_pedacos")}

        # ---- latencias: a chegada SEPARADA pela fase em que a venda foi
        # cometida. A do «durante» nao e transporte, e a queda inteira -- ela
        # vai junto so para o numero dizer quanto o central ficou para tras.
        lat = {}
        for c in caixas:
            for i, (k, quando, fase) in est.cometidas[c].items():
                ch = est.chegada[c].get(i)
                if ch is None:
                    continue
                # A venda cometida com o central no ar e que o SIGKILL pegou
                # antes de ser puxada: e a janela da queda, e nao a latencia
                # de chegada -- misturada, ela fazia o p95 do «antes» dizer
                # 10,7 s num ensaio cuja mediana era 0,6 s.
                if fase == "antes" and ch > t_morte:
                    fase = "antes_pega_pela_queda"
                lat.setdefault(fase, []).append((ch - quando) * 1000)
        r["chegada_ms"] = {f: faixa(v) for f, v in lat.items()}
        r["venda_no_caixa_ms"] = {f: faixa(v, 2) for f, v in est.lat_venda.items()}
        r["vendas_por_fase"] = {f: len(v) for f, v in est.lat_venda.items()}
        r["recusadas"] = len(est.recusadas)
        r["recusadas_por_fase"] = {}
        for _, f, _ in est.recusadas:
            r["recusadas_por_fase"][f] = r["recusadas_por_fase"].get(f, 0) + 1
        r["recusadas_exemplos"] = est.recusadas[:5]
        r["sonda_central_ms"] = {f: {k: faixa(v, 2) for k, v in s.items()}
                                 for f, s in est.sonda.items()}
        r["rodada_do_observador_ms"] = faixa(est.rodadas_poller, 2)

        # ---- o diario do caixa
        por_venda, por_venda_total = [], []
        for c in caixas:
            l1, t1 = bytes_do_diario(os.path.join(b.dir(c), "base", c))
            nv = len(est.cometidas[c])
            if nv:
                por_venda.append((l1 - diario0[c][0]) / nv)
                por_venda_total.append((t1 - diario0[c][1]) / nv)
        r["diario_bytes_por_venda"] = faixa(por_venda, 0)
        r["disco_do_database_bytes_por_venda"] = faixa(por_venda_total, 0)
        itens_med = (sum(sum(x[0] for x in est.cometidas[c].values()) for c in caixas)
                     / max(1, r["conferencia"]["vendas_cometidas"]))
        r["itens_por_venda_media"] = round(itens_med, 2)
        r["bancada_bytes"] = sum(
            os.path.getsize(os.path.join(rz, a))
            for rz, _, arqs in os.walk(BASE) for a in arqs)
        return r
    finally:
        b.derrubar_tudo()


def binario():
    head = subprocess.run(["git", "-C", RAIZ, "rev-parse", "--short", "HEAD"],
                          capture_output=True, text=True).stdout.strip()
    suja = subprocess.run(["git", "-C", RAIZ, "status", "--porcelain", "--", "crates"],
                          capture_output=True, text=True).stdout.strip()
    versao = subprocess.run([PHXSQLD, "--version"], capture_output=True,
                            text=True).stdout.strip()
    # Medidor com binario velho mede o passado: o binario tem de ser mais
    # novo que todo fonte do servidor e do motor.
    mt_bin = os.path.getmtime(PHXSQLD)
    mais_novo = 0.0
    for sub in ("crates",):
        for rz, _, arqs in os.walk(os.path.join(RAIZ, sub)):
            for a in arqs:
                if a.endswith(".rs") or a == "Cargo.toml":
                    mais_novo = max(mais_novo, os.path.getmtime(os.path.join(rz, a)))
    if mais_novo > mt_bin:
        raise SystemExit("o phxsqld e mais velho que o fonte -- rode "
                         "`CARGO_INCREMENTAL=0 cargo build --release -p phxsql-server`")
    return {"commit": head, "arvore_de_crates_suja": bool(suja), "versao": versao,
            "binario_compilado_em": time.strftime("%Y-%m-%d %H:%M:%S",
                                                  time.localtime(mt_bin))}


def agregar(voltas):
    def junta(chave_fn):
        xs = [chave_fn(v) for v in voltas]
        xs = [x for x in xs if x is not None]
        if not xs:
            return None
        return {"min": min(xs), "mediana": statistics.median(xs), "max": max(xs),
                "por_volta": xs}
    g = lambda *ks: (lambda v: _get(v, ks))  # noqa: E731
    return {
        "chegada_com_o_central_no_ar_ms": {
            f: {q: junta(g("chegada_ms", f, q)) for q in ("min", "mediana", "p95", "max")}
            for f in ("antes", "depois")},
        "chegada_das_vendas_da_queda_max_ms": junta(g("chegada_ms", "durante", "max")),
        "vendas_pegas_pela_queda_antes_de_puxadas": junta(
            g("chegada_ms", "antes_pega_pela_queda", "n")),
        "venda_no_caixa_mediana_ms": {f: junta(g("venda_no_caixa_ms", f, "mediana"))
                                      for f in ("antes", "durante", "depois")},
        "venda_no_caixa_max_ms": {f: junta(g("venda_no_caixa_ms", f, "max"))
                                  for f in ("antes", "durante", "depois")},
        "recusadas": junta(g("recusadas")),
        "central_atende_s": junta(g("central_atende_s")),
        "alcance_s": junta(g("alcance_s")),
        "vendas_no_religar": junta(g("vendas_no_religar")),
        "sonda_varrer_max_ms": {f: junta(g("sonda_central_ms", f, "varrer", "max"))
                                for f in ("ocioso", "antes", "depois")},
        "sonda_varrer_mediana_ms": {f: junta(g("sonda_central_ms", f, "varrer", "mediana"))
                                    for f in ("ocioso", "antes", "depois")},
        "sonda_ping_max_ms": {f: junta(g("sonda_central_ms", f, "ping", "max"))
                              for f in ("ocioso", "antes", "depois")},
        "alcance_eventos_aplicados_pelo_central": junta(g("aplicados_pelo_central")),
        "diario_bytes_por_venda_mediana": junta(g("diario_bytes_por_venda", "mediana")),
        "meias_vendas_vistas": junta(g("conferencia", "meias_vendas_vistas")),
        "tres_tabelas_iguais": [v["conferencia"]["tres_tabelas_iguais_caixa_e_central"]
                                for v in voltas],
    }


def _get(d, ks):
    for k in ks:
        if not isinstance(d, dict) or k not in d:
            return None
        d = d[k]
    return d


def main():
    argv = sys.argv[1:]

    def opcao(nome, padrao):
        if nome in argv:
            i = argv.index(nome)
            v = int(argv[i + 1])
            del argv[i:i + 2]
            return v
        return padrao
    n_caixas = opcao("--caixas", 20)
    seg = opcao("--fase", 20)
    n_voltas = int(argv[0]) if argv else 3
    if not os.path.exists(PHXSQLD):
        raise SystemExit(f"nao achei {PHXSQLD}")
    portao = subprocess.run(["bash", os.path.join(RAIZ, "bancada", "esta-medindo.sh")],
                            capture_output=True, text=True)
    if portao.returncode == 0:
        raise SystemExit("ha medicao em curso na maquina -- espere:\n" + portao.stdout)
    bin_ = binario()
    voltas = []
    for i in range(n_voltas):
        if livre() < PISO_LIVRE:
            raise SystemExit(f"disco livre {livre() / 1e9:.2f} GB abaixo do piso")
        print(f"volta {i + 1}/{n_voltas} ...", flush=True)
        r = volta(n_caixas, seg, i + 1)
        print(json.dumps({k: r.get(k) for k in ("chegada_ms", "venda_no_caixa_ms",
                                            "recusadas", "alcance_s",
                                            "central_atende_s")}, ensure_ascii=False))
        print(json.dumps(r["conferencia"], ensure_ascii=False)[:600], flush=True)
        voltas.append(r)
    if os.path.exists(BASE):
        shutil.rmtree(BASE)
    saida = {
        "pedido": 678,
        "quando": time.strftime("%Y-%m-%d %H:%M"),
        "binario": bin_,
        "maquina": f"{os.cpu_count()} nucleos; {n_caixas + 1} processos phxsqld "
                   "em 127.0.0.1, no mesmo container",
        "maquina_ocupada": portao.returncode == 0,
        "reconectar_em_s": RECONECTAR_EM,
        "chegada_inclui": (f"o sono do laco da replica do central ({RECONECTAR_EM} s "
                           "quando a rodada anterior nao achou nada) MAIS o transporte "
                           "MAIS a resolucao do observador (rodada_do_observador_ms)"),
        "carga": {"caixas": n_caixas, "segundos_por_fase": seg,
                  "itens_por_venda": "3 a 10", "pausa_entre_vendas_s": list(PAUSA),
                  "venda": "begin; 1 vendas; k itens (FK conferida); k atualizar "
                           "estoque; commit"},
        "faixas": agregar(voltas),
        "voltas": voltas,
    }
    # Ensaio (menos caixas, fase curta) nao sobrescreve o resultado da casa.
    arq = os.environ.get("PHX_CAIXA_SAIDA") or os.path.join(AQUI, "resultados.json")
    with open(arq + ".parcial", "w") as f:
        json.dump(saida, f, indent=2, ensure_ascii=False)
        f.write("\n")
    os.replace(arq + ".parcial", arq)
    print(f"gravado em {arq}")
    print(json.dumps(saida["faixas"], ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
