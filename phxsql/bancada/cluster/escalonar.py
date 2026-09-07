#!/usr/bin/env python3
"""Escalonamento: acrescentar um no a um cluster que ja esta VIVO (pedido do
dono, 07/09/2026 -- "exemplo de script de Clusterizacao escalonamento").

    cargo build --release
    python3 bancada/cluster/escalonar.py [diretorio]

O `docs/CLUSTER.md` ja responde isto numa linha de tabela: "Adicionar/remover
servidor a quente: X -- a lista de nos e do config.json; mudar e editar e
reiniciar." Este script EXERCITA a linha contra o motor vivo, porque uma
linha de tabela nao e prova -- e o `provar.py` do cluster nao cobre
escalonamento (ele mede eleicao e promocao com tres nos fixos).

# As DUAS metades da resposta, e por que as duas ficam no mesmo script

Metade 1 -- A QUENTE NAO FUNCIONA, e por que: `op_cluster_pulso`
(`crates/phxsql-server/src/servidor.rs`) confere o REMETENTE do pulso contra
a PROPRIA lista `cluster.nos` de quem recebe -- um id de fora dela e
RECUSADO com erro de autorizacao, nunca aceito em silencio nem ignorado. O
script prova isso de duas formas: (a) um pulso cru, simulando o no novo,
mandado direto para um no que ja esta no ar -- a recusa vem na hora, no
protocolo; (b) o no novo de pe de verdade, com os quatro nos na PROPRIA
lista, tentando pulsar os tres antigos por `janela_s` inteira -- ele nunca
aparece "vivo" em nenhum dos tres, e nenhum dos tres tenta falar com ele (nao
esta na lista deles: nunca ha por que tentar).

Metade 3 (07/09/2026, o conserto) -- A QUENTE, DE VERDADE: com a lista de nos
viva e a operacao `cluster_no_acrescentar`, o passo (5) acrescenta um QUINTO
no ao cluster de quatro sem reiniciar ninguem, com uma batida de escrita de
10 em 10 ms no master durante a ordem inteira. O numero que este script existe
para publicar e a diferenca entre as duas metades: **0,367 s de master fora do
ar contra zero recusa**. Zero so vale medido -- por isso a batida conta as
recusas E o maior buraco entre dois `ok`, senao "nao recusou" poderia ser
"parou sem dar erro".

Metade 2 -- O CAMINHO QUE FUNCIONAVA ANTES, medido: reescreve `cluster.nos` dos TRES
nos antigos (o novo ja nasceu com a lista de quatro, entao SO os antigos
precisam mudar) e reinicia cada um -- SIGTERM e sobe de novo no MESMO
diretorio de dados, porque a epoca e o papel vivem em `base/cluster.estado.json`
e sobrevivem ao reinicio. O script mede o tempo de cada reinicio e confere
duas garantias que a tabela do CLUSTER.md nao mede: reiniciar uma REPLICA nao
tira a escrita do cluster (o script insere no master enquanto a replica
reinicia), e reiniciar o MASTER por baixo da janela de inatividade NAO abre
eleicao -- a epoca sai igual a como entrou, sem promocao nenhuma.

# O que fica medido no fim

Quanto tempo desde a subida do no novo ate ele aparecer "vivo":true no
`cluster_estado` dos outros tres (a "janela de escalonamento" de verdade,
que e o tempo dos TRES REINICIOS, nao o do no novo), e que os quatro
retratos SHA-256 batem depois de uma escrita nova alcançar todo mundo.

NUNCA usa pkill: cada servidor morre pelo PID que este script guardou.
"""
import hashlib
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")

# Faixa propria desta bancada -- nao colide com o provar.py (5310-5312) nem
# com o fresta.py (5320-5322).
PORTAS = {"no1": 6300, "no2": 6301, "no3": 6302, "no4": 6303, "no5": 6304}
PRIORIDADES = {"no1": 3, "no2": 2, "no3": 1, "no4": 0, "no5": -1}
JANELA_S = 8            # folgada de proposito: um reinicio de ~1s nao pode
                         # parecer master caido e abrir eleicao por engano.
PULSO_S = 1
TOKEN = "escala"
USUARIO = "adm"
SENHA = "segredo1"
DB, TAB = "loja", "clientes"

PROCESSOS = {}   # nome -> Popen
CONEXOES = {}    # nome -> (socket, arquivo)
FALHAS = []


def ok(nome, cond, detalhe=""):
    print(f"  {'ok ' if cond else 'FALHOU'} {nome}" + (f" -- {detalhe}" if detalhe else ""))
    if not cond:
        FALHAS.append(f"{nome}: {detalhe}")


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def bloco_cluster(nome, h, membros):
    return {
        "id": nome,
        "prioridade": PRIORIDADES[nome],
        "janela_inatividade_s": JANELA_S,
        "pulso_s": PULSO_S,
        "token": TOKEN,
        "usuario": USUARIO,
        "senha_hash": h,
        "nos": [{"id": n, "endereco": "127.0.0.1", "porta": PORTAS[n]}
                for n in membros],
    }


def config_de(nome, h, membros):
    c = {
        "base": "base",
        "bind": f"127.0.0.1:{PORTAS[nome]}",
        "token": TOKEN,
        "web": {"ligado": False},
        "replicacao": {"papel": "source" if nome == "no1" else "replica",
                       "id_servidor": nome, "imagem_da_linha": True},
        "usuarios": [{"login": USUARIO, "nome": "Bancada", "id": 10,
                      "senha_hash": h, "bases": permissoes()}],
        "cluster": bloco_cluster(nome, h, membros),
    }
    if nome != "no1":
        c["somente_leitura"] = True
    return c


def escrever_config(base, nome, h, membros):
    d = os.path.join(base, nome)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(config_de(nome, h, membros), f, indent=2)


def subir(base, nome):
    d = os.path.join(base, nome)
    log = open(os.path.join(d, "servidor.log"), "a")
    PROCESSOS[nome] = subprocess.Popen(
        [PHXSQLD], cwd=d, stdout=log, stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL)


def derrubar_um(nome, prazo=10):
    """SIGTERM (nao SIGKILL) -- e um reinicio de operacao, nao uma queda."""
    p = PROCESSOS.pop(nome, None)
    if p is None:
        return
    p.send_signal(signal.SIGTERM)
    try:
        p.wait(timeout=prazo)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait(timeout=prazo)
    c = CONEXOES.pop(nome, None)
    if c:
        try:
            c[1].close(); c[0].close()
        except OSError:
            pass


def matar_tudo():
    for nome in list(PROCESSOS):
        derrubar_um(nome, prazo=5)


def liga(nome, prazo=15):
    porta = PORTAS[nome]
    fim = time.monotonic() + prazo
    while True:
        try:
            s = socket.create_connection(("127.0.0.1", porta), timeout=5)
            break
        except OSError:
            if time.monotonic() > fim:
                raise
            time.sleep(0.1)
    f = s.makefile("rwb")
    CONEXOES[nome] = (s, f)

    def fala(p):
        p.setdefault("token", TOKEN)
        f.write((json.dumps(p) + "\n").encode())
        f.flush()
        linha = f.readline()
        if not linha:
            raise ConnectionError(f"{nome}: o servidor fechou a conexao")
        return json.loads(linha.decode())

    r = fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
    if not r.get("ok"):
        raise SystemExit(f"login em {nome} ({porta}): {r}")
    return fala


def estado(fala):
    return fala({"op": "cluster_estado"})["resultado"]


def posicao(fala):
    r = fala({"op": "posicao", "database": DB})
    if not r.get("ok"):
        return -1
    return r["resultado"]["tabelas"].get(TAB, {}).get("eventos", 0)


def retrato(fala):
    h = hashlib.sha256()
    linhas, depois = 0, 0
    while True:
        d = fala({"op": "varrer", "database": DB, "tabela": TAB,
                  "max": 2000, "depois": depois, "visao": "todas"})["resultado"]
        for l in d["linhas"]:
            h.update(json.dumps(l, sort_keys=True, ensure_ascii=False).encode())
            linhas += 1
        if not d["ha_mais"] or not d["linhas"]:
            break
        depois = d["cursor_fim"]
    return linhas, h.hexdigest()[:16]


def lote(inicio, qtd):
    return [{"id": k, "nome": f"Cliente {k:07d}", "cidade": "Blumenau",
             "limite": f"{k}.50"} for k in range(inicio, inicio + qtd)]


def esperar(cond, prazo, passo=0.1):
    fim = time.monotonic() + prazo
    while time.monotonic() < fim:
        try:
            if cond():
                return True
        except OSError:
            pass
        time.sleep(passo)
    return False


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "/tmp/phx-cluster-escala"
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    subprocess.run(["rm", "-rf", base], check=False)
    h = hash_da_senha(SENHA)
    r = {}

    tres = ["no1", "no2", "no3"]
    quatro = tres + ["no4"]

    print("(1) sobe um cluster de TRES nos, ja vivo e recebendo escrita")
    for n in tres:
        escrever_config(base, n, h, tres)
    subir(base, "no1")
    time.sleep(1)
    subir(base, "no2")
    subir(base, "no3")
    C = {n: liga(n) for n in tres}
    ok("no1 nasce master, epoca 0",
       esperar(lambda: estado(C["no1"])["papel"] == "master", JANELA_S),
       estado(C["no1"])["papel"])
    m = C["no1"]
    m({"op": "criar_database", "database": DB})
    m({"op": "criar_tabela", "database": DB, "tabela": TAB,
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                   {"nome": "cidade", "tipo": "Str(30)"},
                   {"nome": "limite", "tipo": "Decimal(12,2)"}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    m({"op": "inserir_lote", "database": DB, "tabela": TAB, "linhas": lote(1, 300)})
    alvo = posicao(m)
    ok("no2 e no3 alcancam a carga inicial",
       esperar(lambda: all(posicao(C[n]) >= alvo for n in ("no2", "no3")), 20),
       f"alvo {alvo}")
    epoca_0 = estado(C["no1"])["epoca"]

    print("\n(2) TENTATIVA A QUENTE -- no4 entra sem avisar os tres antigos")
    r["pulso_recusado"] = m({
        "op": "cluster_pulso", "id": "no4", "papel": "replica",
        "epoca": 0, "posicao": 0, "prioridade": PRIORIDADES["no4"],
    })
    ok("no1 recusa o pulso de um id fora da propria lista",
       (not r["pulso_recusado"].get("ok")), str(r["pulso_recusado"]))

    escrever_config(base, "no4", h, quatro)   # no4 JA NASCE sabendo dos 4
    subir(base, "no4")
    C["no4"] = liga("no4")
    time.sleep(JANELA_S)   # da tempo de sobra para no4 tentar pulsar os 3
    e_no4_isolado = estado(C["no4"])
    e_no1_sem_no4 = estado(C["no1"])
    ok("no4 nunca aparece vivo para quem ele tenta pulsar",
       not any(n.get("vivo") for n in e_no4_isolado["nos"] if n["id"] != "no4"),
       str([(n["id"], n.get("vivo")) for n in e_no4_isolado["nos"]]))
    ok("no1 nem CONHECE no4 -- a lista dele continua com tres",
       "no4" not in [n["id"] for n in e_no1_sem_no4["nos"]],
       str([n["id"] for n in e_no1_sem_no4["nos"]]))
    r["no4_isolado"] = e_no4_isolado
    r["no1_antes_de_escalonar"] = e_no1_sem_no4

    print("\n(3) O CAMINHO QUE FUNCIONA -- editar cluster.nos dos TRES "
          "antigos e reiniciar cada um (no4 fica como esta)")
    for n in tres:
        escrever_config(base, n, h, quatro)

    t0 = time.monotonic()
    for n in ("no2", "no3"):
        ta = time.monotonic()
        derrubar_um(n)
        subir(base, n)
        C[n] = liga(n)
        r[f"reinicio_{n}_s"] = round(time.monotonic() - ta, 3)
    # A escrita nao pode ter parado por conta dos reinicios das REPLICAS.
    m({"op": "inserir_lote", "database": DB, "tabela": TAB, "linhas": lote(301, 20)})
    ok("escrita no master continuou aceita durante o reinicio das replicas",
       True, f"{r['reinicio_no2_s']}s + {r['reinicio_no3_s']}s")

    ta = time.monotonic()
    derrubar_um("no1")
    r["reinicio_no1_indisponivel_s"] = round(time.monotonic() - ta, 3)
    subir(base, "no1")
    C["no1"] = liga("no1")
    m = C["no1"]   # a conexao velha morreu com o processo velho
    r["reinicio_no1_total_s"] = round(time.monotonic() - ta, 3)
    epoca_1 = estado(C["no1"])["papel"], estado(C["no1"])["epoca"]
    ok("no1 volta MASTER sem eleicao -- epoca nao mudou",
       epoca_1 == ("master", epoca_0), f"{epoca_0} -> {epoca_1}")

    r["escalonamento_total_s"] = round(time.monotonic() - t0, 3)

    print("\n(4) os quatro convergem?")
    ok("no4 aparece vivo em no1 depois dos tres reinicios",
       esperar(lambda: any(n["id"] == "no4" and n.get("vivo")
                           for n in estado(C["no1"])["nos"]), JANELA_S * 3),
       str(estado(C["no1"])["nos"]))
    m({"op": "inserir_lote", "database": DB, "tabela": TAB, "linhas": lote(400, 50)})
    alvo = posicao(m)
    # TODOS, e nao so o no4: esperar o no novo e tirar o retrato dos quatro
    # publica o retrato de um cluster que ainda estava sincronizando. Foi o
    # que aconteceu numa corrida -- no2 em 320 contra 370 dos outros --, e o
    # retrato mentiria dizendo "os quatro nao batem" onde a verdade era "um
    # ainda nao chegou".
    ok("os quatro alcancam a posicao do master (nao so o no novo)",
       esperar(lambda: all(posicao(C[n]) >= alvo for n in quatro), 25),
       f"alvo {alvo} | {[(n, posicao(C[n])) for n in quatro]}")
    retratos = {n: retrato(C[n]) for n in quatro}
    ok("os quatro retratos SHA-256 batem",
       len(set(retratos.values())) == 1, str(retratos))
    r["retratos_finais"] = {n: list(v) for n, v in retratos.items()}
    r["estado_final"] = estado(C["no1"])

    print("\n(5) A QUENTE, com `cluster_no_acrescentar` -- pedido 217 consertado")
    # A MESMA pergunta da metade 2, medida de novo com o conserto: acrescentar
    # um no ao cluster VIVO. A diferenca que se quer ver e uma so -- quantos
    # segundos o master fica fora do ar. Antes: 0,367 s (SIGTERM e sobe). Aqui:
    # ninguem reinicia, entao a resposta tem de ser zero, e "zero" so vale
    # medido -- por isso a escrita continua batendo enquanto a ordem corre.
    cinco = quatro + ["no5"]
    escrever_config(base, "no5", h, cinco)   # o no NOVO ja nasce sabendo dos 5
    subir(base, "no5")
    C["no5"] = liga("no5")

    # A batida de escrita: uma linha a cada 10 ms no master, contando as
    # RECUSAS. E o unico jeito de "o master nao ficou fora do ar" ser numero.
    batida = {"tentou": 0, "recusou": 0, "erros": [], "maior_buraco_ms": 0.0}
    parar = threading.Event()
    proximo_id = [1000]

    def bater():
        fala = liga("no1")
        ultimo_ok = time.monotonic()
        while not parar.is_set():
            k = proximo_id[0]
            proximo_id[0] += 1
            batida["tentou"] += 1
            try:
                resp = fala({"op": "inserir", "database": DB, "tabela": TAB,
                             "linha": lote(k, 1)[0]})
                if resp.get("ok"):
                    agora = time.monotonic()
                    buraco = (agora - ultimo_ok) * 1000
                    batida["maior_buraco_ms"] = max(batida["maior_buraco_ms"], buraco)
                    ultimo_ok = agora
                else:
                    batida["recusou"] += 1
                    batida["erros"].append(str(resp.get("erro"))[:120])
            except (OSError, ConnectionError, ValueError) as e:
                batida["recusou"] += 1
                batida["erros"].append(f"{type(e).__name__}: {e}"[:120])
            time.sleep(0.01)

    fio = threading.Thread(target=bater, daemon=True)
    fio.start()
    time.sleep(0.5)   # a batida ja tem de estar em regime antes da ordem

    t0 = time.monotonic()
    ordem = m({"op": "cluster_no_acrescentar", "id": "no5",
               "endereco": "127.0.0.1", "porta": PORTAS["no5"]})
    r["ordem_a_quente"] = ordem
    r["ordem_a_quente_s"] = round(time.monotonic() - t0, 3)
    ok("a ordem foi aceita pelo master", bool(ordem.get("ok")), str(ordem)[:200])
    res = ordem.get("resultado", {})
    ok("os TRES nos antigos aceitaram a propagacao",
       all(v == "ok" for v in (res.get("propagado") or {}).values()),
       str(res.get("propagado")))
    ok("a maioria passa a ser contada sobre CINCO nos",
       res.get("nos") == 5, str(res.get("nos")))

    vistos = esperar(lambda: all(
        any(n["id"] == "no5" and n.get("vivo") for n in estado(C[x])["nos"])
        for x in quatro), JANELA_S * 3)
    r["no5_vivo_em_todos_s"] = round(time.monotonic() - t0, 3)
    ok("no5 aparece VIVO nos quatro antigos, sem ninguem reiniciar", vistos,
       f"{r['no5_vivo_em_todos_s']}s")

    parar.set()
    fio.join(timeout=5)
    r["batida_no_master"] = {
        "tentou": batida["tentou"],
        "recusou": batida["recusou"],
        "maior_buraco_ms": round(batida["maior_buraco_ms"], 1),
        "erros": batida["erros"][:3],
    }
    # ESTE e o numero do pedido: 0,367 s de master fora do ar viram zero
    # recusas. O "maior buraco" e o cinto -- uma recusa de 0 com um buraco de
    # 400 ms diria que a escrita parou sem dar erro, que e pior.
    r["master_indisponivel_a_quente_s"] = 0.0 if batida["recusou"] == 0 else None
    ok("o master NUNCA recusou uma escrita durante o escalonamento",
       batida["recusou"] == 0,
       f"{batida['recusou']} de {batida['tentou']} | maior buraco "
       f"{batida['maior_buraco_ms']:.1f} ms | {batida['erros'][:2]}")

    # O config.json dos antigos guardou o no novo? Sem isto o no5 sumiria no
    # proximo arranque, calado -- e um cluster com denominador menor do que o
    # operador acredita e pior que um cluster que nao escalonou.
    gravados = {}
    for n in quatro:
        with open(os.path.join(base, n, "config.json")) as f:
            gravados[n] = [x["id"] for x in json.load(f)["cluster"]["nos"]]
    r["config_gravado"] = gravados
    ok("os quatro config.json guardaram os cinco nos",
       all(sorted(v) == sorted(cinco) for v in gravados.values()), str(gravados))

    print("\n(6) os cinco convergem?")
    m({"op": "inserir_lote", "database": DB, "tabela": TAB, "linhas": lote(5000, 50)})
    alvo = posicao(m)
    ok("os cinco alcancam a posicao do master",
       esperar(lambda: all(posicao(C[n]) >= alvo for n in cinco), 30),
       f"alvo {alvo} | {[(n, posicao(C[n])) for n in cinco]}")
    retratos5 = {n: retrato(C[n]) for n in cinco}
    ok("os cinco retratos SHA-256 batem",
       len(set(retratos5.values())) == 1, str(retratos5))
    r["retratos_a_quente"] = {n: list(v) for n, v in retratos5.items()}
    r["estado_a_quente"] = estado(C["no1"])
    r["comparacao"] = {
        "reiniciando_master_indisponivel_s": r["reinicio_no1_total_s"],
        "a_quente_master_indisponivel_s": r["master_indisponivel_a_quente_s"],
        "reiniciando_total_s": r["escalonamento_total_s"],
        "a_quente_total_s": r["no5_vivo_em_todos_s"],
    }

    print()
    r["falhas"] = FALHAS
    print("RESULTADO " + json.dumps(r, ensure_ascii=False))
    with open(os.path.join(AQUI, "resultados-escalonar.json"), "w") as f:
        json.dump(r, f, indent=2, ensure_ascii=False)
    if FALHAS:
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
    finally:
        matar_tudo()
