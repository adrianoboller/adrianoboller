#!/usr/bin/env python3
"""Prova o cluster com eleicao e promocao automatica (pedido 126).

    cargo build --release
    python3 bancada/cluster/provar.py [diretorio]

Sobe TRES phxsqld proprios (127.0.0.1:5310-5312) e um SMTP falso (5316) que
captura os avisos. O roteiro e o resultado esperado de cada passo estao no
LEIA-ME.md, escritos ANTES da primeira corrida.

NUNCA usa pkill: cada servidor morre pelo PID que este script guardou. O
phxsqld demo em 5199/5599 nao e tocado.
"""
import base64
import hashlib
import json
import os
import socket
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")

PORTAS = {"no1": 5310, "no2": 5311, "no3": 5312}
PRIORIDADES = {"no1": 0, "no2": 2, "no3": 1}
SMTP_PORTA = 5316
TOKEN = "pulso"
USUARIO = "adm"
SENHA = "segredo1"
JANELA_S = 4

PROCESSOS = {}   # nome -> Popen (SO os nossos; matar e por PID daqui)
FALHAS = []


def ok(nome, cond, detalhe=""):
    print(f"  {'ok ' if cond else 'FALHOU'} {nome}" + (f" -- {detalhe}" if detalhe else ""))
    if not cond:
        FALHAS.append(f"{nome}: {detalhe}")


# ------------------------------------------------------------- SMTP falso

EMAILS = []      # {"assunto":..., "corpo":..., "quando": time.monotonic()}


def smtp_falso():
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", SMTP_PORTA))
    srv.listen(8)

    def atender(cx):
        f = cx.makefile("rwb")

        def diz(s):
            f.write((s + "\r\n").encode())
            f.flush()

        try:
            diz("220 smtp-falso da bancada")
            dados, em_data = [], False
            while True:
                linha = f.readline()
                if not linha:
                    break
                t = linha.decode(errors="replace").rstrip("\r\n")
                if em_data:
                    if t == ".":
                        em_data = False
                        guardar(dados)
                        dados = []
                        diz("250 recebido")
                    else:
                        dados.append(t)
                    continue
                cmd = t.upper()
                if cmd.startswith("DATA"):
                    em_data = True
                    diz("354 manda")
                elif cmd.startswith("QUIT"):
                    diz("221 tchau")
                    break
                else:
                    diz("250 ok")
        except OSError:
            pass
        finally:
            cx.close()

    def guardar(linhas):
        assunto, corpo_b64, no_corpo = "", [], False
        for l in linhas:
            if no_corpo:
                corpo_b64.append(l)
            elif l == "":
                no_corpo = True
            elif l.lower().startswith("subject:"):
                assunto = l.split(":", 1)[1].strip()
        try:
            corpo = base64.b64decode("".join(corpo_b64)).decode()
        except Exception:
            corpo = "\n".join(corpo_b64)
        EMAILS.append({"assunto": assunto, "corpo": corpo,
                       "quando": time.monotonic()})

    def laco():
        while True:
            try:
                cx, _ = srv.accept()
            except OSError:
                return
            threading.Thread(target=atender, args=(cx,), daemon=True).start()

    threading.Thread(target=laco, daemon=True).start()


# ------------------------------------------------------------- servidores

def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def bloco_cluster(nome, h):
    return {
        "id": nome,
        "prioridade": PRIORIDADES[nome],
        "janela_inatividade_s": JANELA_S,
        "pulso_s": 1,
        "avisar_cada_min": 0.1,
        "token": TOKEN,
        "usuario": USUARIO,
        "senha_hash": h,
        "nos": [{"id": n, "endereco": "127.0.0.1", "porta": p}
                for n, p in PORTAS.items()],
        "email": {"ligado": True, "servidor": "127.0.0.1",
                  "porta": SMTP_PORTA, "de": "phx@bancada.local",
                  "para": ["dba@bancada.local"], "timeout_s": 5},
    }


def config_de(nome, h, com_cluster=True, origens=None):
    c = {
        "base": "base",
        "bind": f"127.0.0.1:{PORTAS[nome]}",
        "token": TOKEN,
        "web": {"ligado": False},
        "replicacao": {"papel": "source" if nome == "no1" else "replica",
                       "id_servidor": nome, "imagem_da_linha": True},
        "usuarios": [{"login": USUARIO, "nome": "Bancada", "id": 10,
                      "senha_hash": h, "bases": permissoes()}],
    }
    if nome != "no1":
        c["somente_leitura"] = True
    if com_cluster:
        c["cluster"] = bloco_cluster(nome, h)
    if origens is not None:
        c["replicacao"]["origens"] = origens
    return c


def escrever_config(base, nome, cfg):
    d = os.path.join(base, nome)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)


def subir(base, nome):
    d = os.path.join(base, nome)
    log = open(os.path.join(d, "servidor.log"), "a")
    PROCESSOS[nome] = subprocess.Popen(
        [PHXSQLD], cwd=d, stdout=log, stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL)


def matar(nome):
    p = PROCESSOS.pop(nome, None)
    if p is None:
        return
    p.kill()          # pelo PID nosso; nada de pkill
    p.wait(timeout=10)


def matar_tudo():
    for nome in list(PROCESSOS):
        matar(nome)


# --------------------------------------------------------------- clientes

def liga(porta, prazo=15):
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

    def fala(p):
        p.setdefault("token", TOKEN)
        f.write((json.dumps(p) + "\n").encode())
        f.flush()
        return json.loads(f.readline().decode())

    r = fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
    if not r.get("ok"):
        raise SystemExit(f"login na porta {porta}: {r}")
    return fala


def estado(fala):
    return fala({"op": "cluster_estado"})["resultado"]


def posicao(fala):
    r = fala({"op": "posicao", "database": "loja"})
    if not r.get("ok"):
        return -1
    return r["resultado"]["tabelas"].get("clientes", {}).get("eventos", 0)


def retrato(fala):
    h = hashlib.sha256()
    linhas, depois = 0, 0
    while True:
        d = fala({"op": "varrer", "database": "loja", "tabela": "clientes",
                  "max": 2000, "depois": depois, "visao": "todas"})["resultado"]
        for l in d["linhas"]:
            h.update(json.dumps(l, sort_keys=True,
                                ensure_ascii=False).encode())
            linhas += 1
        if not d["ha_mais"] or not d["linhas"]:
            break
        depois = d["cursor_fim"]
    return linhas, h.hexdigest()[:16]


def inserir(fala, linhas):
    return fala({"op": "inserir_lote", "database": "loja",
                 "tabela": "clientes", "linhas": linhas})


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


# ----------------------------------------------------------------- passos

def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "/tmp/phx-cluster"
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    subprocess.run(["rm", "-rf", base], check=False)
    smtp_falso()
    h = hash_da_senha(SENHA)
    r = {}

    print("(a) tres nos sobem; cluster_estado igual nos tres")
    for nome in PORTAS:
        escrever_config(base, nome, config_de(nome, h))
    subir(base, "no1")
    time.sleep(1)
    for nome in ("no2", "no3"):
        subir(base, nome)
    C = {n: liga(p) for n, p in PORTAS.items()}
    time.sleep(3)  # dois pulsos: todo mundo se apresenta
    es = {n: estado(f) for n, f in C.items()}
    ok("master unico e e o no1",
       all(e["master"] and e["master"]["id"] == "no1" for e in es.values()),
       str({n: e["master"] for n, e in es.items()}))
    ok("epoca 0 nos tres", all(e["epoca"] == 0 for e in es.values()))
    ok("tres vivos na visao de cada um",
       all(sum(1 for x in e["nos"] if x["vivo"]) == 3 for e in es.values()))

    print("(b) escreve no master; replicas acompanham; REDIRECIONA na replica")
    m = C["no1"]
    m({"op": "criar_database", "database": "loja"})
    m({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                   {"nome": "cidade", "tipo": "Str(30)"},
                   {"nome": "limite", "tipo": "Decimal(12,2)"}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    rr = inserir(m, lote(1, 3000))
    ok("carga inicial gravada", rr.get("ok", False), str(rr)[:120])
    alvo = posicao(m)
    ok("replicas alcancam a carga",
       esperar(lambda: all(posicao(C[n]) >= alvo for n in ("no2", "no3")), 20),
       f"alvo {alvo}, no2={posicao(C['no2'])}, no3={posicao(C['no3'])}")
    retratos = {n: retrato(f) for n, f in C.items()}
    ok("retratos identicos nos tres", len(set(retratos.values())) == 1,
       str(retratos))
    rec = inserir(C["no2"], lote(90000, 1))
    ok("escrita na replica redireciona para 5310",
       (not rec.get("ok")) and rec.get("nome") == "REDIRECIONA"
       and "REDIRECIONA 127.0.0.1:5310" in rec.get("erro", ""),
       str(rec)[:160])
    r["redireciona"] = rec.get("erro", "")

    print("(c) mata o no1 pelo PID; mede a promocao do no2")
    matar("no1")
    del C["no1"]
    t0 = time.monotonic()
    promovido = esperar(
        lambda: estado(C["no2"])["papel"] == "master", JANELA_S * 3)
    t_promocao = time.monotonic() - t0
    ok("no2 se promoveu", promovido, f"{t_promocao:.1f}s")
    aceita = esperar(lambda: inserir(C["no2"], lote(10001, 1)).get("ok",
                                                                   False),
                     JANELA_S * 2)
    t_escrita = time.monotonic() - t0
    ok("novo master aceita escrita", aceita, f"{t_escrita:.1f}s")
    r["promocao_s"] = round(t_promocao, 1)
    r["escrita_aceita_s"] = round(t_escrita, 1)
    rr = inserir(C["no2"], lote(10100, 500))
    alvo = posicao(C["no2"])
    ok("no3 segue o novo master",
       esperar(lambda: posicao(C["no3"]) >= alvo, 20),
       f"alvo {alvo}, no3={posicao(C['no3'])}")
    rec = inserir(C["no3"], lote(90001, 1))
    ok("REDIRECIONA do no3 aponta o novo master (5311)",
       "REDIRECIONA 127.0.0.1:5311" in rec.get("erro", ""), str(rec)[:160])
    e3 = estado(C["no3"])
    ok("epoca subiu para 1 no no3", e3["epoca"] == 1, str(e3["epoca"]))

    print("(d) e-mails: degradacao repetida a cada 6s e promocao unica")
    time.sleep(14)
    promos = [e for e in EMAILS if "promocao" in e["assunto"]]
    degras = [e for e in EMAILS if "degradado" in e["assunto"]]
    caidos = [e for e in degras if "no1" in e["corpo"]]
    ok("exatamente 1 e-mail de promocao", len(promos) == 1,
       f"{len(promos)} capturados")
    ok("degradacao cita o no1 caido", len(caidos) >= 1, f"{len(caidos)}")
    por_no = {}
    for e in degras:
        # O corpo diz "O cluster ... O no noX ve:" -- e o remetente.
        remetente = e["corpo"].split(" ve:")[0].split()[-1] \
            if " ve:" in e["corpo"] else "?"
        por_no.setdefault(remetente, []).append(e["quando"])
    repete = any(len(v) >= 2 for v in por_no.values())
    ok("aviso repete no mesmo no (a cada ~6s)", repete,
       str({k: len(v) for k, v in por_no.items()}))
    r["emails_promocao"] = len(promos)
    r["emails_degradacao"] = len(degras)

    print("(f) o antigo master volta e se rebaixa sozinho")
    subir(base, "no1")
    C["no1"] = liga(PORTAS["no1"])
    rebaixou = esperar(
        lambda: estado(C["no1"])["papel"] == "replica"
        and (estado(C["no1"])["master"] or {}).get("id") == "no2",
        JANELA_S * 4)
    ok("no1 voltou como replica do no2", rebaixou,
       str(estado(C["no1"])["master"]))
    inserir(C["no2"], lote(10700, 200))
    alvo = posicao(C["no2"])
    ok("no1 alcanca o que perdeu",
       esperar(lambda: posicao(C["no1"]) >= alvo, 20),
       f"alvo {alvo}, no1={posicao(C['no1'])}")

    print("(e) particao sem maioria: o no3 sozinho NAO se promove")
    n_emails = len(EMAILS)
    matar("no2")
    matar("no1")
    del C["no2"], C["no1"]
    time.sleep(JANELA_S * 3)
    e3 = estado(C["no3"])
    ok("no3 continua replica", e3["papel"] == "replica", e3["papel"])
    ok("degradado diz que NAO promove",
       any("NAO promovo" in d for d in e3["degradado"]), str(e3["degradado"]))
    rec = inserir(C["no3"], lote(95000, 1))
    ok("escrita recusada no no isolado, sem apontar um master morto",
       (not rec.get("ok")) and rec.get("nome") != "REDIRECIONA",
       str(rec)[:160])
    sem_maioria = [e for e in EMAILS[n_emails:]
                   if "maioria" in e["corpo"] or "NAO promovo" in e["corpo"]]
    ok("e-mail de sem-maioria capturado", len(sem_maioria) >= 1,
       f"{len(sem_maioria)}")
    r["no3_nao_promoveu"] = e3["papel"] == "replica"

    print("    sobe no1 e no2 de volta: o cluster sara sozinho")
    subir(base, "no2")
    subir(base, "no1")
    C["no2"] = liga(PORTAS["no2"])
    C["no1"] = liga(PORTAS["no1"])
    ok("no2 volta master pela epoca persistida",
       esperar(lambda: estado(C["no2"])["papel"] == "master", JANELA_S * 4),
       estado(C["no2"])["papel"])
    inserir(C["no2"], lote(11000, 100))
    alvo = posicao(C["no2"])
    ok("os tres convergem",
       esperar(lambda: all(posicao(C[n]) >= alvo for n in ("no1", "no3")), 25),
       f"alvo {alvo}")
    retratos = {n: retrato(f) for n, f in C.items()}
    ok("retratos finais identicos", len(set(retratos.values())) == 1,
       str(retratos))
    r["linhas_no_fim"] = retratos["no2"][0]

    print("(g) sem o bloco cluster, tudo como hoje")
    matar_tudo()
    C.clear()
    n_emails = len(EMAILS)
    subprocess.run(["rm", "-rf", base], check=False)
    origem = [{"nome": "no1", "host": "127.0.0.1", "porta": PORTAS["no1"],
               "token": TOKEN, "usuario": USUARIO, "senha_hash": h,
               "databases": ["loja"], "reconectar_em": 1}]
    escrever_config(base, "no1", config_de("no1", h, com_cluster=False))
    for nome in ("no2", "no3"):
        escrever_config(base, nome,
                        config_de(nome, h, com_cluster=False, origens=origem))
    subir(base, "no1")
    time.sleep(1)
    subir(base, "no2")
    subir(base, "no3")
    C = {n: liga(p) for n, p in PORTAS.items()}
    m = C["no1"]
    m({"op": "criar_database", "database": "loja"})
    m({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                   {"nome": "cidade", "tipo": "Str(30)"},
                   {"nome": "limite", "tipo": "Decimal(12,2)"}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    inserir(m, lote(1, 500))
    alvo = posicao(m)
    ok("replicacao classica continua funcionando",
       esperar(lambda: all(posicao(C[n]) >= alvo for n in ("no2", "no3")), 20),
       f"alvo {alvo}")
    rc = m({"op": "cluster_estado"})
    ok("cluster_estado sem cluster e erro claro",
       (not rc.get("ok")) and "cluster" in rc.get("erro", ""), str(rc)[:140])
    rec = inserir(C["no2"], lote(90000, 1))
    ok("replica sem cluster recusa como sempre (somente leitura)",
       (not rec.get("ok")) and rec.get("nome") == "ACESSO_NEGADO",
       str(rec)[:140])
    time.sleep(8)
    ok("nenhum e-mail novo na fase sem cluster", len(EMAILS) == n_emails,
       f"{len(EMAILS) - n_emails} chegaram")
    r["sem_cluster_nada_muda"] = len(EMAILS) == n_emails

    print()
    r["falhas"] = FALHAS
    print("RESULTADO " + json.dumps(r, ensure_ascii=False))
    with open(os.path.join(AQUI, "resultados.json"), "w") as f:
        json.dump(r, f, indent=2, ensure_ascii=False)
    if FALHAS:
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
    finally:
        matar_tudo()
