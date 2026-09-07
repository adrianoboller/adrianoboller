#!/usr/bin/env python3
"""O CANAL ABERTO do cluster: o que ele leva hoje, em que direcao, e a que custo.

    cargo build --release
    python3 bancada/quorum/canal.py [voltas]

# A premissa que este medidor existe para conferir

O pedido 207 decidiu construir o quorum de escrita "pela rota do CANAL
ABERTO", e a linha do PENDENCIAS aponta o `bidirecional.rs` como onde essa
rota mora. **Ler o fonte desmente a metade da premissa e confirma a outra**, e
as duas metades precisavam de numero:

1. **`bidirecional.rs` NAO tem canal nenhum.** O cabecalho do modulo diz, na
   primeira linha, "a parte funda da replicacao bidirecional, **sem rede**":
   ele resolve conflito por carimbo, decide quem vence e evita o laco pela
   origem no evento. Nao abre soquete, nao conecta, nao empurra. Procurar a
   rota ali e procurar no lugar errado.

2. **O canal aberto EXISTE -- e e o do PULSO.** O `laco_do_pulso`
   (`servidor.rs`) abre uma conexao de CADA no para CADA outro e a mantem viva
   num laco, trocando `cluster_pulso` a cada `pulso_s`. Num cluster de N nos
   sao N*(N-1) conexoes longas, e entre elas estao as do MASTER para cada
   replica -- abertas pelo master, ja autenticadas, ja quentes.

E dai sai o achado que muda o desenho do 207: **a premissa "o master nao tem
como fazer ninguem buscar" e verdadeira na REPLICACAO e falsa no CLUSTER.**
Na replicacao pura o source so recebe conexao; num cluster, o proprio pulso ja
obriga todo mundo a alcancar todo mundo, e o master ja segura a conexao de
saida que um push usaria. A propriedade de firewall que o desenho comprou nao
se perde: ela ja tinha sido gasta pelo cluster, e nao por este pedido.

# O que se mede aqui, e por que estes tres numeros

- **canais abertos** -- contados na telemetria de cada no (`pulso-<id>`), e
  nao deduzidos da formula. Fio que a telemetria nao mostra e fio que nao
  existe.
- **pulso numa conexao QUENTE** -- o piso do canal: o que uma ida e volta
  custa sem carregar dado nenhum. Nenhum push pode custar menos que isto.
- **empurrar UM evento numa conexao QUENTE** -- `aplicar` com o evento da
  linha recem-gravada, pela mesma conexao ja aberta e ja autenticada. E o que
  um commit por quorum pagaria POR REPLICA, e a diferenca para o `levar` de
  0,475 ms da `medir.py` e justamente o aperto de mao que aquela media incluia.

Tudo em `127.0.0.1`, entao isto e o PISO: a rede real custa mais.

NUNCA usa pkill: cada servidor morre pelo PID que este script guardou.
"""
import json
import os
import signal
import socket
import statistics
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")

# Faixa 7200-7299 da frente F2, sem colidir com a `medir.py` (5900-5902) nem
# com a `escalonar.py` (6300-6304).
PORTAS = {"no1": 7210, "no2": 7211, "no3": 7212}
PRIORIDADE = {"no1": 3, "no2": 2, "no3": 1}
TOKEN, USUARIO, SENHA = "canal", "adm", "segredo1"
DB, TAB = "loja", "clientes"
JANELA_S, PULSO_S = 10, 1

PROCESSOS, CONEXOES = {}, {}


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def config_de(base, nome, h, membros):
    c = {
        "base": "base",
        "bind": f"127.0.0.1:{PORTAS[nome]}",
        "token": TOKEN,
        "replicacao": {"papel": "source" if nome == "no1" else "replica",
                       "id_servidor": nome, "imagem_da_linha": True},
        "usuarios": [{"login": USUARIO, "nome": "Bancada", "id": 10,
                      "senha_hash": h,
                      # Supervisor porque a telemetria (que conta os canais)
                      # exige administrador do SERVIDOR, e nao de uma base.
                      "supervisor": True,
                      "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                                      "excluir": True, "criar": True, "administrar": True,
                                      "diario": True, "verificar": True, "replicar": True}}}],
        "cluster": {"id": nome, "prioridade": PRIORIDADE[nome],
                    "janela_inatividade_s": JANELA_S, "pulso_s": PULSO_S,
                    "token": TOKEN, "usuario": USUARIO, "senha_hash": h,
                    "nos": [{"id": n, "endereco": "127.0.0.1", "porta": PORTAS[n]}
                            for n in membros]},
    }
    if nome != "no1":
        c["somente_leitura"] = True
    return c


def subir(base, nome, h, membros):
    d = os.path.join(base, nome)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(config_de(base, nome, h, membros), f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    PROCESSOS[nome] = subprocess.Popen(
        [PHXSQLD], cwd=d, stdout=log, stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL)


def matar_tudo():
    for nome, p in list(PROCESSOS.items()):
        try:
            p.send_signal(signal.SIGTERM)
            p.wait(timeout=6)
        except Exception:
            try:
                p.kill()
            except Exception:
                pass
        PROCESSOS.pop(nome, None)


def liga(nome, prazo=20):
    """Uma conexao QUENTE: aberta, autenticada, e reusada em todas as voltas.

    E o que o `laco_do_pulso` faz -- e por isso medir com conexao nova a cada
    volta mediria o aperto de mao, que o master ja pagou uma vez so."""
    fim = time.monotonic() + prazo
    while True:
        try:
            s = socket.create_connection(("127.0.0.1", PORTAS[nome]), timeout=5)
            break
        except OSError:
            if time.monotonic() > fim:
                raise
            time.sleep(0.1)
    s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
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
        raise SystemExit(f"login em {nome}: {r}")
    return fala


def corpo(r):
    if not r.get("ok"):
        raise SystemExit(f"pedido recusado: {r}")
    return r.get("resultado", {})


def canais_abertos(fala):
    """Os fios `pulso-<id>` que a telemetria DESTE no mostra vivos.

    Contados, e nao deduzidos de N*(N-1): fio que a telemetria nao mostra e
    fio que nao existe, e a formula continuaria certa depois de o laco morrer."""
    t = corpo(fala({"op": "telemetria", "amostras": 1}))
    fios = t.get("fios") or t.get("threads") or []
    # O `pulso-supervisor` NAO e canal: ele nao conecta em ninguem, so cuida
    # das threads que conectam. Conta-lo inflaria o numero em um por no, e um
    # medidor que conta o proprio zelador como canal mente para mais.
    return sorted(f.get("nome", "") for f in fios
                  if str(f.get("nome", "")).startswith("pulso-")
                  and f.get("nome") != "pulso-supervisor")


def med(x):
    return round(statistics.median(x), 3)


def faixa(x):
    return [round(min(x), 3), round(max(x), 3)]


def main():
    voltas = int(sys.argv[1]) if len(sys.argv) > 1 else 60
    base = "/tmp/phx-f2-canal"
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    subprocess.run(["rm", "-rf", base], check=False)
    h = hash_da_senha(SENHA)
    nos = ["no1", "no2", "no3"]
    for n in nos:
        subir(base, n, h, nos)
        time.sleep(0.4)
    C = {n: liga(n) for n in nos}
    m = C["no1"]

    # Espera o pulso circular: medir antes dele mediria um cluster que ainda
    # nao existe.
    fim = time.monotonic() + JANELA_S * 2
    while time.monotonic() < fim:
        e = corpo(m({"op": "cluster_estado"}))
        if sum(1 for x in e["nos"] if x.get("vivo")) == 3:
            break
        time.sleep(0.2)

    r = {"voltas": voltas}
    r["canais_por_no"] = {n: canais_abertos(C[n]) for n in nos}
    r["canais_abertos"] = sum(len(v) for v in r["canais_por_no"].values())
    r["canais_do_master"] = r["canais_por_no"]["no1"]

    m({"op": "criar_database", "database": DB})
    m({"op": "criar_tabela", "database": DB, "tabela": TAB,
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    # A replica precisa da tabela antes do primeiro `aplicar`, senao o medidor
    # cronometraria o erro em vez do trabalho.
    for n in ("no2", "no3"):
        fim = time.monotonic() + 20
        while time.monotonic() < fim:
            t = C[n]({"op": "tabelas", "database": DB})
            if t.get("ok") and any(x.get("nome", x) == TAB if isinstance(x, dict) else x == TAB
                                   for x in corpo(t).get("tabelas", corpo(t))):
                break
            time.sleep(0.3)

    pulso, empurrar, dois, tres = [], [], [], []
    posicao = {n: 0 for n in ("no2", "no3")}
    for v in range(voltas):
        # (a) o PISO do canal: uma ida e volta sem dado nenhum, na conexao
        # quente -- exatamente o que o `pulsar` manda.
        t0 = time.perf_counter()
        C["no2"]({"op": "cluster_pulso", "id": "no1", "papel": "master",
                  "epoca": 0, "posicao": v, "prioridade": 3})
        pulso.append((time.perf_counter() - t0) * 1000)

        # (b) grava no master e EMPURRA por cada canal quente, cronometrando
        # cada confirmacao. E o que um commit por quorum pagaria por replica.
        m({"op": "inserir", "database": DB, "tabela": TAB,
           "valores": {"id": 10_000 + v, "nome": f"n{v}"}})
        marcas = []
        for n in ("no2", "no3"):
            ta = time.perf_counter()
            ev = corpo(m({"op": "replicar", "database": DB, "tabela": TAB,
                          "desde": posicao[n], "max": 500}))
            eventos = ev.get("eventos") or []
            if not eventos:
                sys.exit(f"ZERO EVENTOS na volta {v} para {n}: a replica puxou "
                         "sozinha e este medidor mediria o proprio concorrente.")
            ap = C[n]({"op": "aplicar", "database": DB, "tabela": TAB,
                       "eventos": eventos})
            if not ap.get("ok"):
                sys.exit(f"aplicar falhou na volta {v} em {n}: {ap}")
            marcas.append((time.perf_counter() - ta) * 1000)
            posicao[n] = ev.get("ate", posicao[n])
        empurrar.extend(marcas)
        # 3 nos: o master ja e um voto. 2-de-3 espera a PRIMEIRA confirmacao.
        dois.append(min(marcas))
        tres.append(max(marcas))

    r["pulso_quente_ms"] = med(pulso)
    r["pulso_quente_faixa"] = faixa(pulso)
    r["empurrar_um_evento_ms"] = med(empurrar)
    r["empurrar_faixa"] = faixa(empurrar)
    r["quorum_2de3_pelo_canal_ms"] = med(dois)
    r["quorum_3de3_pelo_canal_ms"] = med(tres)
    r["bidirecional_tem_canal"] = False
    r["onde_o_canal_mora"] = "servidor.rs :: laco_do_pulso/pulsar (cluster_pulso)"
    r["maquina"] = "tres phxsqld em 127.0.0.1, no mesmo container"
    r["aviso_da_maquina"] = ("TUDO em localhost: a rede real custa mais, e "
                             "estes numeros sao o PISO")
    r["medido_em"] = time.strftime("%Y-%m-%d %H:%M")

    print("RESULTADO " + json.dumps(r, ensure_ascii=False, indent=1))
    with open(os.path.join(AQUI, "resultados-canal.json"), "w") as f:
        json.dump(r, f, indent=1, ensure_ascii=False)


if __name__ == "__main__":
    try:
        main()
    finally:
        matar_tudo()
