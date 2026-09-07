#!/usr/bin/env python3
"""Os TRES jeitos de cancelar uma transacao travada, pelo SOQUETE.

    python3 bancada/transacoes/travada.py

Pergunta do dono, 07/09/2026: *«testar como cancelar uma transacao atomica
travada»*. `bancada/transacoes/provar.py` ja prova o desenho inteiro (BEGIN,
COMMIT, ROLLBACK, queda de conexao, SQL, diario); esta sonda isola só o que
"travada" quer dizer -- uma conexao A que abriu `begin`, mexeu numa linha e nao
mandou `commit` nem `rollback` -- e exercita os TRES jeitos documentados de
tirar essa linha da frente, em `docs/TRANSACOES.md` §4.7 e §9 e em
`docs/TELEMETRIA.md` §4:

1. **pela propria conexao**: A manda `rollback`.
2. **pelo prazo, sozinho**: o `TIMEOUT` da transacao inteira estoura e o
   GESTOR aborta -- nenhuma thread e morta (`docs/TRANSACOES.md` §4.7).
3. **pelo administrador**: `telemetria_encerrar` (que devolve `ociosa` para
   uma transacao parada NO MEIO, porque ela nao esta dentro de um laco com
   ponto de cancelamento -- `docs/TELEMETRIA.md` §4.4) e `encerrar_sessao`,
   que fecha o soquete e cai na MESMA rede de protecao da queda de conexao.

Cada cenario comeca do MESMO valor gravado (100) e termina medindo o valor
com uma conexao QUE NUNCA travou nada -- para provar que o cancelamento nao
deixou a escrita pela metade.

Mata so os PIDs que subiu. Nunca `pkill -f`.
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = f"/tmp/phx-transacoes-travada-{os.getpid()}"
PORTA = int(os.environ.get("PHX_SONDA_PORTA", "6720"))
TOKEN = "travada"

PIDS = []
FALHAS = []


def ok(nome, condicao, detalhe=""):
    marca = "OK   " if condicao else "FALHA"
    print(f"  {marca} {nome}" + (f"  -- {detalhe}" if detalhe else ""))
    if not condicao:
        FALHAS.append(nome)


class Ligacao:
    def __init__(self, porta=PORTA):
        self.s = socket.create_connection(("127.0.0.1", porta), 10)
        self.s.settimeout(10)
        self.f = self.s.makefile("rwb")

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r

    def morrer(self):
        # OS DOIS descritores, e nessa ordem -- `makefile()` segura o fd, e
        # fechar so o soquete deixa o servidor sem ver o fim da conexao.
        try:
            self.f.close()
        except OSError:
            pass
        try:
            self.s.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.s.close()


def subir():
    os.makedirs(BASE + "/dados", exist_ok=True)
    with open(BASE + "/config.json", "w") as f:
        json.dump(
            {
                "bind": f"127.0.0.1:{PORTA}",
                "base": BASE + "/dados",
                "token": TOKEN,
                "web": {"ligado": False},
                # O padrao de 5 min tornaria o cenario 2 impraticavel de
                # provar aqui -- e por isso o `begin` desta sonda manda
                # "timeout" por pedido em vez de mexer nisto. Fica em 1
                # (o MINIMO: e inteiro, em MINUTOS -- nao aceita fracao,
                # ver o texto da resposta U) so para o config nao mentir
                # sobre o padrao do servidor que subiu.
                "recursos": {"transacao_prazo_min": 1},
            },
            f,
        )
    log = open(BASE + "/servidor.log", "a")
    p = subprocess.Popen(
        [PHXSQLD, "--config", BASE + "/config.json"],
        stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
    )
    PIDS.append(p.pid)
    fim = time.time() + 15
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.3).close()
            return
        except OSError:
            if p.poll() is not None:
                sys.exit(f"phxsqld morreu no arranque; veja {BASE}/servidor.log")
            time.sleep(0.1)
    sys.exit(f"o servidor nao subiu na porta {PORTA}")


def derrubar():
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    time.sleep(1)
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def montar(c):
    c.fala({"op": "criar_database", "database": "loja"})
    c.fala({"op": "criar_tabela", "database": "loja", "tabela": "contas",
            "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                        {"nome": "saldo", "tipo": "Int8", "obrigatoria": True}],
            "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                         "primario": True}]})
    c.fala({"op": "inserir", "database": "loja", "tabela": "contas",
            "linha": {"id": 1, "saldo": 100}})


def saldo(c):
    """Le pela conexao C, que NUNCA abriu transacao nesta linha -- e o que
    prova que a leitura ve o valor de fato COMETIDO, e nao um rastro do que
    a transacao travada tinha empilhado."""
    r = c.fala({"op": "ler", "database": "loja", "tabela": "contas", "rowid": 1})
    return r.get("saldo")


def repor(c, valor=100):
    c.fala({"op": "atualizar", "database": "loja", "tabela": "contas",
            "rowid": 1, "linha": {"id": 1, "saldo": valor}})


def cenario_1_lock_timeout_e_rollback(c):
    print("\n== 1. LOCK TIMEOUT (a espera de B) + cancelamento PELA PROPRIA CONEXAO (A faz rollback) ==")
    a, b = Ligacao(), Ligacao()
    a.fala({"op": "begin"})
    r = a.fala({"op": "atualizar", "database": "loja", "tabela": "contas",
                "rowid": 1, "linha": {"id": 1, "saldo": 111}})
    ok("A trava a linha (atualizar dentro do begin)", r.get("ok"), json.dumps(r)[:160])

    b.fala({"op": "begin", "lock_timeout": "400ms"})
    comeco = time.time()
    r = b.fala({"op": "atualizar", "database": "loja", "tabela": "contas",
                "rowid": 1, "linha": {"id": 1, "saldo": 222}})
    levou_s = time.time() - comeco
    ok("B espera e recebe o codigo documentado (4005 EM_TRANSACAO)",
       not r.get("ok") and r.get("codigo") == 4005 and r.get("nome") == "EM_TRANSACAO",
       json.dumps(r)[:220])
    ok("a mensagem cita o LOCK TIMEOUT", "LOCK TIMEOUT" in str(r.get("erro", "")),
       str(r.get("erro"))[:200])
    ok("B esperou por volta dos 400 ms declarados", 0.35 <= levou_s <= 2.0,
       f"esperou {levou_s * 1000:.0f} ms")
    print(f"    tempo de espera medido: {levou_s * 1000:.1f} ms")
    print(f"    erro completo de B: {json.dumps({k: r.get(k) for k in ('codigo', 'nome', 'erro', 'repetir')})}")
    b.fala({"op": "rollback"})  # B nunca travou nada; fecha por higiene.

    antes = saldo(c)
    print(f"    saldo ANTES do cancelamento (por uma conexao que nao viu a transacao de A): {antes}")
    r = a.fala({"op": "rollback"})
    ok("A cancela pela propria conexao (ROLLBACK)",
       r.get("ok") and r.get("transaction_state") == "ROLLED_BACK", json.dumps(r)[:160])
    depois = saldo(c)
    print(f"    ROLLBACK de A: {json.dumps({k: r.get(k) for k in ('transaction_state', 'descartadas') if k in r})}")
    ok("depois do ROLLBACK a linha volta a valer 100 (nada de A foi gravado)",
       depois == 100, f"saldo ficou {depois}")
    print(f"    >>> a linha ficou valendo: saldo={depois}")
    a.morrer()
    return depois == 100


def cenario_2_o_prazo_aborta_sozinho(c):
    print("\n== 2. o TIMEOUT da transacao inteira aborta A SOZINHO ==")
    a = Ligacao()
    r = a.fala({"op": "begin", "timeout": "1500ms"})
    ok("A abre com TIMEOUT de 1500 ms (por pedido, sem mexer no config)",
       r.get("ok") and r.get("transaction_state") == "ACTIVE", json.dumps(r)[:200])
    r = a.fala({"op": "atualizar", "database": "loja", "tabela": "contas",
                "rowid": 1, "linha": {"id": 1, "saldo": 333}})
    ok("A trava a linha e NAO manda commit nem rollback", r.get("ok"), json.dumps(r)[:160])

    print("    ... esperando 2,0 s (o TIMEOUT declarado e de 1,5 s) sem A mandar nada ...")
    time.sleep(2.0)

    meio = saldo(c)
    print(f"    saldo enquanto A ainda 'segura' a transacao vencida (ninguem perguntou nada a A ainda): {meio}")

    # So agora A manda a PROXIMA operacao -- e o prazo e conferido "na hora de
    # usar" (docs/TRANSACOES.md secao 4.7), nao por uma thread que fica de olho.
    r = a.fala({"op": "atualizar", "database": "loja", "tabela": "contas",
                "rowid": 1, "linha": {"id": 1, "saldo": 444}})
    ok("a proxima operacao de A recebe 6002 TRANSACAO_ABORTADA",
       not r.get("ok") and r.get("codigo") == 6002 and r.get("nome") == "TRANSACAO_ABORTADA",
       json.dumps(r)[:220])
    print(f"    erro completo: {json.dumps({k: r.get(k) for k in ('codigo', 'nome', 'erro', 'repetir')})}")

    # ABORT_ONLY so sai pelo ROLLBACK -- e o prazo ja soltou as travas e
    # jogou a lista de escrita fora; isto so fecha a ficha da transacao.
    a.fala({"op": "rollback"})
    depois = saldo(c)
    ok("depois do aborto por prazo a linha continua em 100 (nada de A foi gravado)",
       depois == 100, f"saldo ficou {depois}")
    print(f"    >>> a linha ficou valendo: saldo={depois}")
    a.morrer()
    return depois == 100


def cenario_3_o_administrador(c):
    print("\n== 3. cancelamento PELO ADMINISTRADOR: telemetria_encerrar e encerrar_sessao ==")
    a = Ligacao()
    inicio = a.fala({"op": "begin"})
    ligacao_de_a = inicio.get("ligacao")
    ok("peguei o numero de ligacao de A pela propria resposta do begin",
       isinstance(ligacao_de_a, int) and ligacao_de_a > 0, json.dumps(inicio)[:160])
    r = a.fala({"op": "atualizar", "database": "loja", "tabela": "contas",
                "rowid": 1, "linha": {"id": 1, "saldo": 555}})
    ok("A trava a linha e fica OCIOSA (nao manda mais nada)", r.get("ok"), json.dumps(r)[:160])
    time.sleep(0.3)  # o suficiente para A estar "parada esperando pedido"

    print("\n  3a. telemetria_encerrar na atividade de A -- e o limite HONESTO desta ferramenta")
    r = c.fala({"op": "telemetria_encerrar", "id": f"dados:{ligacao_de_a}"})
    ok("o servidor acha a atividade de A (ela existe, so nao esta rodando nada)",
       r.get("ok"), json.dumps(r)[:220])
    ok("o estado e OCIOSA -- a transacao esta parada ENTRE pedidos, nao dentro de um laco",
       r.get("estado") == "ociosa", json.dumps(r)[:220])
    ok("o aviso aponta o caminho certo: encerrar_sessao",
       "encerrar_sessao" in str(r.get("aviso", "")), str(r.get("aviso"))[:200])
    print(f"    resposta: {json.dumps(r)[:300]}")

    meio = saldo(c)
    ok("telemetria_encerrar sozinho NAO desfez nada (a linha continua mostrando o valor de antes)",
       meio == 100, f"saldo ficou {meio}")

    print("\n  3b. encerrar_sessao(id) -- o que REALMENTE cancela a transacao travada")
    r = c.fala({"op": "encerrar_sessao", "id": ligacao_de_a})
    ok("encerrar_sessao aceita o mesmo numero", r.get("ok"), json.dumps(r)[:200])
    print(f"    resposta: {json.dumps(r)[:300]}")

    # A queda da conexao e assincrona do lado do servidor -- espera com PRAZO.
    fim, sumiu = time.time() + 10, False
    while time.time() < fim:
        r = c.fala({"op": "sessoes"})
        if not any(s.get("id") == ligacao_de_a for s in r.get("sessoes", [])):
            sumiu = True
            break
        time.sleep(0.2)
    ok("a sessao de A sumiu da lista (o soquete foi fechado)", sumiu)

    depois = saldo(c)
    ok("depois do encerrar_sessao a linha volta a valer 100 (a queda desfez a transacao)",
       depois == 100, f"saldo ficou {depois}")
    print(f"    >>> a linha ficou valendo: saldo={depois}")

    # A conexao de A ja foi fechada PELO SERVIDOR; o lado do cliente so
    # confirma que o soquete morreu, sem contar como falha do cenario.
    try:
        a.f.close()
        a.s.close()
    except OSError:
        pass
    return depois == 100


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode cargo build --release -p phxsql-server")
    shutil.rmtree(BASE, ignore_errors=True)
    try:
        subir()
        c = Ligacao()
        montar(c)

        r1 = cenario_1_lock_timeout_e_rollback(c)
        repor(c, 100)
        r2 = cenario_2_o_prazo_aborta_sozinho(c)
        repor(c, 100)
        r3 = cenario_3_o_administrador(c)

        print("\n" + "=" * 70)
        if FALHAS:
            print(f"FALHAS ({len(FALHAS)}): " + "; ".join(FALHAS))
            return 1
        print("os TRES cancelamentos provados: rollback, prazo sozinho e "
              f"encerrar_sessao. Linha final em cada um: {r1 and r2 and r3 and 100}")
        return 0
    finally:
        derrubar()
        shutil.rmtree(BASE, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
