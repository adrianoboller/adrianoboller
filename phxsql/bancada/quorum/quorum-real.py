#!/usr/bin/env python3
"""O QUORUM DE ESCRITA DE VERDADE -- pedido 207, medido pelo soquete.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/quorum/quorum-real.py [voltas]

# O que este medidor existe para decidir

O contrato (`docs/propostas/207-e-513p2-contrato-01-10-2026.md` §207.4) fez a
conta do custo por COMPONENTES -- gravar 0,209 + `fsync` local + ida e volta +
aplicar + `fsync` na replica ~= 3,6 ms por commit, ~280 commits/s -- e disse
que a bancada decide. Os numeros de antes (`medir.py`, `canal.py`) NAO tinham
nenhum dos dois `fsync`: o quorum ali era o fio, nao o disco. Aqui e o commit
inteiro, pelo caminho do produto: o `inserir` no master com
`cluster.quorum_minimo` ligado, a replica levando o lote pelo canal que ELA
abriu (`replicar_aguardar`), aplicando, sincronizando, e so entao confirmando.

# As quatro fases, no mesmo cluster

1. **sem quorum** (`quorum_minimo: 0`, a quente) -- a linha de base;
2. **1 de 2** -- o commit espera UMA replica;
3. **2 de 2** -- espera as duas;
4. **SIGKILL** -- as duas replicas mortas pelo PID (o que depende do SO se
   prova contra o SO): o primeiro commit volta no prazo com
   `alcancado:false`, o segundo responde na hora `degradado:true`; depois as
   replicas voltam e se mede quanto o master leva para voltar ao sincrono.

Cada fase traz mediana E faixa min-max (pedido 155: vencedor so com faixas
que nao se cruzam). Tudo em `127.0.0.1` e no disco deste conteiner: o
`fsync` aqui e virtio com cache, e o de um cliente pode custar ordens de
grandeza mais (lacuna do contrato, §207.7).

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

# Faixa 7330-7339 desta bancada, sem colidir com a `canal.py` (7210-7212), a
# `medir.py` (5900-5902) nem a `escalonar.py` (6300-6304).
PORTAS = {"no1": 7330, "no2": 7331, "no3": 7332}
PRIORIDADE = {"no1": 3, "no2": 2, "no3": 1}
TOKEN = "quorum-real"
DB, TAB = "loja", "clientes"
JANELA_S, PULSO_S = 30, 1
PRAZO_MS = 2000

PROCESSOS = {}


def config_de(nome, quorum):
    return {
        "base": "base",
        # bancada de teste, NAO cliente do produto: fala em claro para medir
        # o quorum sem o aperto de mao no meio.
        "bind": f"127.0.0.1:{PORTAS[nome]}", "cifra_fio": {"exigir": False},
        "token": TOKEN,
        "web": {"ligado": False},
        "replicacao": {"papel": "source" if nome == "no1" else "replica",
                       "id_servidor": nome, "imagem_da_linha": True},
        "cluster": {"id": nome, "prioridade": PRIORIDADE[nome],
                    "janela_inatividade_s": JANELA_S, "pulso_s": PULSO_S,
                    "token": TOKEN, "cifra": False,
                    "quorum_minimo": quorum, "quorum_prazo_ms": PRAZO_MS,
                    "nos": [{"id": n, "endereco": "127.0.0.1", "porta": PORTAS[n]}
                            for n in PORTAS]},
    }


def subir(base, nome, quorum):
    d = os.path.join(base, nome)
    os.makedirs(d, exist_ok=True)
    caminho = os.path.join(d, "config.json")
    if not os.path.exists(caminho):
        with open(caminho, "w") as f:
            json.dump(config_de(nome, quorum), f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    PROCESSOS[nome] = subprocess.Popen(
        [PHXSQLD, "--config", caminho], cwd=d, stdout=log,
        stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)


def matar(nome, sinal=signal.SIGKILL):
    p = PROCESSOS.pop(nome, None)
    if p is None:
        return
    try:
        p.send_signal(sinal)
        p.wait(timeout=10)
    except Exception:
        try:
            p.kill()
            p.wait(timeout=5)
        except Exception:
            pass


def matar_tudo():
    for nome in list(PROCESSOS):
        matar(nome, signal.SIGTERM)


class Fio:
    """Uma conexao QUENTE, aberta uma vez: medir com conexao nova a cada volta
    mediria o aperto, que o cliente de verdade paga uma vez so."""

    def __init__(self, nome, prazo=20):
        fim = time.monotonic() + prazo
        while True:
            try:
                self.s = socket.create_connection(("127.0.0.1", PORTAS[nome]), timeout=30)
                break
            except OSError:
                if time.monotonic() > fim:
                    raise
                time.sleep(0.1)
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.f = self.s.makefile("rwb")

    def __call__(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise ConnectionError("o servidor fechou a conexao")
        return json.loads(linha.decode())

    def fechar(self):
        # `makefile` segura o descritor: fechar so o soquete deixaria o fd
        # aberto e o servidor nunca veria o fim (licao do BULKINSERT).
        try:
            self.f.close()
        finally:
            self.s.close()


def corpo(r):
    if not r.get("ok"):
        raise SystemExit(f"pedido recusado: {r}")
    return r.get("resultado", {})


def med(x):
    return round(statistics.median(x), 3)


def faixa(x):
    return [round(min(x), 3), round(max(x), 3)]


def estado_quorum(m):
    return corpo(m({"op": "replicacao_estado"})).get("quorum") or {}


def esperar(cond, prazo, rotulo):
    fim = time.monotonic() + prazo
    while time.monotonic() < fim:
        if cond():
            return
        time.sleep(0.1)
    raise SystemExit(f"PAROU: {rotulo} nao aconteceu em {prazo} s")


def fase(m, voltas, inicio, exigir):
    cliente, servidor = [], []
    for v in range(voltas):
        t0 = time.perf_counter()
        r = corpo(m({"op": "inserir", "database": DB, "tabela": TAB,
                     "linha": {"id": inicio + v}}))
        cliente.append((time.perf_counter() - t0) * 1000)
        q = r.get("quorum")
        if exigir:
            if not q or not q.get("alcancado"):
                raise SystemExit(f"PAROU: volta {v} sem quorum alcancado: {r}")
            servidor.append(q.get("ms", 0.0))
        elif q is not None:
            raise SystemExit(f"PAROU: quorum desligado e a resposta trouxe o campo: {r}")
    saida = {"mediana_ms": med(cliente), "faixa_ms": faixa(cliente),
             "commits_por_s": round(1000.0 / statistics.mean(cliente), 1)}
    if servidor:
        saida["espera_no_servidor_mediana_ms"] = med(servidor)
    return saida


def main():
    voltas = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    base = "/tmp/phx-207-quorum-real"
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode o build de release antes")
    subprocess.run(["rm", "-rf", base], check=False)
    for n in PORTAS:
        subir(base, n, 1)
        time.sleep(0.3)
    m = Fio("no1")
    m({"op": "criar_database", "database": DB})
    corpo(m({"op": "criar_tabela", "database": DB, "tabela": TAB,
             "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True}],
             "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                          "primario": True}]}))
    esperar(lambda: len(estado_quorum(m).get("replicas") or {}) == 2, 60,
            "as duas replicas abrirem o canal do quorum")

    def quorum(n):
        corpo(m({"op": "config_gravar", "campos": {"cluster.quorum_minimo": n}}))
        # Ligar a quente com a replica no meio de um pull (o master respondia
        # «desligado» e ela voltou a puxar) pode degradar o PRIMEIRO commit:
        # o pull dela pede a trava que o commit segura. Tres pulsos bastam
        # para ela voltar ao canal -- e e o que o MANUAL manda esperar.
        time.sleep(3 * PULSO_S)

    r = {"voltas": voltas, "prazo_ms": PRAZO_MS}
    # Aquecimento: a primeira gravacao cria a tabela nas replicas.
    fase(m, 5, 1, True)
    quorum(0)
    r["sem_quorum"] = fase(m, voltas, 1_000, False)
    quorum(1)
    r["quorum_1_de_2"] = fase(m, voltas, 10_000, True)
    quorum(2)
    r["quorum_2_de_2"] = fase(m, voltas, 20_000, True)
    base_ms = r["sem_quorum"]["mediana_ms"]
    r["vezes_1_de_2"] = round(r["quorum_1_de_2"]["mediana_ms"] / base_ms, 2)
    r["vezes_2_de_2"] = round(r["quorum_2_de_2"]["mediana_ms"] / base_ms, 2)
    e = estado_quorum(m)
    r["arquivos_sincronizados_antes_de_esperar"] = e.get(
        "arquivos_sincronizados_antes_de_esperar")

    # SIGKILL nas duas, com quorum 1: o SO derruba o processo, e o master so
    # descobre pela falta da confirmacao.
    quorum(1)
    matar("no2")
    matar("no3")
    t0 = time.perf_counter()
    a = corpo(m({"op": "inserir", "database": DB, "tabela": TAB, "linha": {"id": 90_000}}))
    r["sigkill_primeiro_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    r["sigkill_primeiro"] = a.get("quorum")
    r["sigkill_aviso"] = a.get("aviso_quorum")
    t0 = time.perf_counter()
    b = corpo(m({"op": "inserir", "database": DB, "tabela": TAB, "linha": {"id": 90_001}}))
    r["degradado_segundo_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    r["degradado_segundo"] = b.get("quorum")
    if (r["sigkill_primeiro"] or {}).get("alcancado") is not False:
        raise SystemExit(f"PAROU: com as duas mortas o quorum disse alcancado: {a}")
    if not (r["degradado_segundo"] or {}).get("degradado"):
        raise SystemExit(f"PAROU: o segundo commit nao veio degradado: {b}")

    # As replicas voltam: quanto ate o master voltar ao sincrono (recuo +
    # alcance), e o commit seguinte volta a esperar.
    t0 = time.monotonic()
    subir(base, "no2", 1)
    subir(base, "no3", 1)
    esperar(lambda: estado_quorum(m).get("estado") == "sincrono", 120,
            "o master voltar ao sincrono")
    r["volta_ao_sincrono_s"] = round(time.monotonic() - t0, 2)
    c = corpo(m({"op": "inserir", "database": DB, "tabela": TAB, "linha": {"id": 90_002}}))
    r["depois_da_volta"] = c.get("quorum")
    e = estado_quorum(m)
    r["degradacoes"] = e.get("degradacoes")
    r["maquina"] = "tres phxsqld (release) em 127.0.0.1, no mesmo conteiner, ext4 em /dev/vda"
    r["aviso_da_maquina"] = ("TUDO em localhost e no mesmo disco: a rede real e o "
                             "fsync de outro disco custam mais")
    r["medido_em"] = time.strftime("%Y-%m-%d %H:%M")
    m.fechar()
    print("RESULTADO " + json.dumps(r, ensure_ascii=False, indent=1))
    with open(os.path.join(AQUI, "resultados-quorum-real.json"), "w") as f:
        json.dump(r, f, indent=1, ensure_ascii=False)


if __name__ == "__main__":
    try:
        main()
    finally:
        matar_tudo()
