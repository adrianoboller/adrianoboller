#!/usr/bin/env python3
"""A prova REAL das transacoes, pelo SOQUETE.

    python3 bancada/transacoes/provar.py

Teste unitario nao prova o servidor: soquete prova. Esta casa ja pagou por
isso -- os dez testes do `BULKINSERT` passavam, e a prova pelo soquete mostrou
que a queda da conexao nao soltava a reserva.

# O que so ele acha

Os testes de `servidor.rs` exercitam a transacao pela API interna, com a
`Sessao` montada a mao. Cinco coisas nenhum deles alcanca:

1. a transacao morre com a CONEXAO de verdade -- e a armadilha ja paga aqui e
   que `socket.makefile()` do Python segura o descritor: fechar so o soquete
   deixa o fd aberto, e o servidor nunca ve o fim da conexao. Este script
   fecha os DOIS;
2. **SIGKILL no meio de um COMMIT**: o processo morre com a marca `.tx` no
   disco, e o banco tem de reabrir sabendo dizer o que aconteceu;
3. duas CONEXOES de verdade disputando a mesma linha, com o LOCK TIMEOUT
   valendo em relogio de parede;
4. o comportamento VELHO pelo soquete: um cliente que nunca manda BEGIN;
5. o SQL pela porta de dados, com as clausulas declaradas.

Cada teste que este script faz tem PRAZO: um que trave em vez de falhar
travaria a bateria inteira.

Mata so os PIDs que ele mesmo subiu. Nunca `pkill -f`.
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
RAIZ = sys.argv[1] if len(sys.argv) > 1 else os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = "/tmp/phx-transacoes-prova"

PORTA = 7320
TOKEN = "txprova"
USUARIO = "adm"
SENHA = "segredo1"

PIDS = []
FALHAS = []


def ok(nome, condicao, detalhe=""):
    if not condicao:
        FALHAS.append(f"{nome}: {detalhe}")
    print(("  OK   " if condicao else "  FALHA") + f"  {nome}" +
          (f"  -- {detalhe}" if detalhe else ""))


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def escrever_config(h):
    os.makedirs(BASE, exist_ok=True)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump({
            "base": "base",
            "bind": f"127.0.0.1:{PORTA}",
            "token": TOKEN,
            "web": {"ligado": False},
            "recursos": {"transacao_prazo_min": 1,
                         "transacao_lock_timeout_ms": 300},
            "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                          "senha_hash": h,
                          "bases": {"*": {"ler": True, "inserir": True,
                                          "alterar": True, "excluir": True,
                                          "criar": True, "administrar": True,
                                          "diario": True, "verificar": True,
                                          "replicar": True}}}],
        }, f, indent=2)


def subir():
    log = open(os.path.join(BASE, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    PIDS.append(p.pid)
    return p


def esperar_porta(porta, prazo=20):
    fim = time.time() + prazo
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", porta), 0.5).close()
            return True
        except OSError:
            time.sleep(0.2)
    return False


def esperar_porta_fechar(porta, prazo=20):
    fim = time.time() + prazo
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", porta), 0.3).close()
            time.sleep(0.2)
        except OSError:
            return True
    return False


class Ligacao:
    """Uma conexao de verdade, que sabe MORRER de verdade.

    O `makefile()` segura o descritor: fechar so o soquete deixa o fd aberto e
    o servidor nunca ve o fim da conexao. Foi assim que a prova do BULKINSERT
    passou por engano, e um teste que passa por engano e pior que um que falta.
    """

    def __init__(self, porta=PORTA):
        self.s = socket.create_connection(("127.0.0.1", porta))
        self.s.settimeout(20)
        self.f = self.s.makefile("rwb")
        r = self.fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
        if not r.get("ok"):
            raise SystemExit(f"login: {r}")

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def morrer(self):
        # OS DOIS, e nessa ordem.
        try:
            self.f.close()
        except OSError:
            pass
        try:
            self.s.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.s.close()


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


def marcas():
    d = os.path.join(BASE, "base", "loja")
    if not os.path.isdir(d):
        return []
    return [n for n in os.listdir(d) if n.endswith(".tx")]


def montar(c):
    c.fala({"op": "criar_database", "database": "loja"})
    for t in ("clientes", "pedidos"):
        c.fala({"op": "criar_tabela", "database": "loja", "tabela": t,
                "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                            {"nome": "nome", "tipo": "Str(40)"}],
                "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                             "primario": True}]})


def quantas(c, tabela):
    """Quantas linhas ATIVAS a tabela tem, pelo contador do proprio motor.

    Contar as linhas de um `varrer` mentiria: o `max` e limitado pelo
    `max_linhas` do servidor (mil, por padrao), e uma tabela de tres mil
    linhas apareceria com mil. Foi o que este script mediu errado antes -- e
    o numero errado acusou de defeito uma recuperacao que estava certa.
    """
    r = c.fala({"op": "esquema", "database": "loja", "tabela": tabela})
    return r["resultado"]["registros"] if r.get("ok") else -1


def visiveis(c, tabela, teto=1000):
    """As linhas que um `varrer` devolve -- para quando o que importa e o que
    a LEITURA ve, e nao o que a tabela tem."""
    r = c.fala({"op": "varrer", "database": "loja", "tabela": tabela, "max": teto})
    return len(r["resultado"]["linhas"]) if r.get("ok") else -1


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode cargo build --release")
    shutil.rmtree(BASE, ignore_errors=True)
    escrever_config(hash_da_senha(SENHA))
    subir()
    if not esperar_porta(PORTA):
        derrubar()
        sys.exit("o servidor nao subiu")

    c = Ligacao()
    montar(c)

    print("\n== 1. o comportamento VELHO: quem nunca manda BEGIN ==")
    r = c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": 1, "nome": "Ana"}})
    ok("inserir sem transacao grava na hora",
       r.get("ok") and quantas(c, "clientes") == 1, json.dumps(r)[:160])
    r = c.fala({"op": "transacao"})
    ok("estado IDLE e resposta, nao erro",
       r.get("ok") and r["resultado"]["transaction_state"] == "IDLE",
       json.dumps(r)[:160])

    print("\n== 2. BEGIN, INSERT, ROLLBACK: nenhum slot queimado ==")
    antes = c.fala({"op": "esquema", "database": "loja", "tabela": "clientes"})["resultado"]["slots"]
    c.fala({"op": "begin"})
    for i in range(100, 200):
        c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": i, "nome": f"c{i}"}})
    meio = visiveis(c, "clientes")
    r = c.fala({"op": "rollback"})
    depois = c.fala({"op": "esquema", "database": "loja", "tabela": "clientes"})["resultado"]["slots"]
    # Ate o pedido 162 esta linha afirmava `meio == 1` -- a leitura NAO via o
    # que a propria transacao tinha empilhado, e faltava o I do ACID. O
    # SP000006 entrou em 02/09 e trocou o comportamento; esta asercao nao foi
    # atras, e passou dois dias reprovando sem ninguem ver, porque a bateria
    # inteira so voltou a rodar em 04/09.
    #
    # A asercao de hoje continua valendo NOS DOIS SENTIDOS, que e o que ela
    # tem de fazer: se a `Sobreposicao` for desligada, a leitura volta a ver 1
    # e este passo reprova. Conferir `meio > 1` nao serviria -- passaria com a
    # sobreposicao meia funcionando.
    ok("a transacao ve o que ela mesma empilhou", meio == 101, f"viu {meio}")
    ok("o rollback descarta as 100", r["resultado"]["descartadas"] == 100,
       json.dumps(r)[:160])
    ok("nenhum slot queimado", antes == depois, f"{antes} -> {depois}")

    print("\n== 3. COMMIT: a lista inteira, com o rowid prometido ==")
    c.fala({"op": "begin"})
    prometidos = []
    for i in range(200, 250):
        r = c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                    "linha": {"id": i, "nome": f"c{i}"}})
        prometidos.append(r["resultado"]["rowid"])
    r = c.fala({"op": "commit"})
    ok("gravou as 50", r["resultado"]["gravadas"] == 50, json.dumps(r)[:160])
    ok("o rowid prometido e o gravado",
       prometidos == list(range(2, 52)), f"{prometidos[:3]}...{prometidos[-1]}")
    ok("a tabela tem 51", quantas(c, "clientes") == 51)

    print("\n== 4. duas CONEXOES na mesma linha, com o LOCK TIMEOUT de verdade ==")
    a, b = Ligacao(), Ligacao()
    a.fala({"op": "begin"})
    b.fala({"op": "begin"})
    a.fala({"op": "atualizar", "database": "loja", "tabela": "clientes",
            "rowid": 2, "linha": {"id": 200, "nome": "do A"}})
    # Linha DIFERENTE: passa sem esperar nada.
    r = b.fala({"op": "atualizar", "database": "loja", "tabela": "clientes",
                "rowid": 3, "linha": {"id": 201, "nome": "do B"}})
    ok("linha diferente nao espera", r.get("ok"), json.dumps(r)[:160])
    # MESMA linha: espera o LOCK TIMEOUT e devolve EM_TRANSACAO.
    comeco = time.time()
    r = b.fala({"op": "atualizar", "database": "loja", "tabela": "clientes",
                "rowid": 2, "linha": {"id": 200, "nome": "tambem do B"}})
    levou = time.time() - comeco
    ok("mesma linha devolve EM_TRANSACAO",
       not r.get("ok") and r.get("nome") == "EM_TRANSACAO", json.dumps(r)[:200])
    ok("e ele pede repeticao", r.get("repetir") is True, json.dumps(r)[:160])
    ok("esperou o LOCK TIMEOUT de 300 ms", 0.25 <= levou <= 3.0, f"{levou:.3f}s")
    a.fala({"op": "rollback"})
    b.fala({"op": "rollback"})

    print("\n== 5. a QUEDA DA CONEXAO desfaz e solta ==")
    d = Ligacao()
    d.fala({"op": "begin"})
    d.fala({"op": "inserir", "database": "loja", "tabela": "pedidos",
            "linha": {"id": 1, "nome": "some junto"}})
    r = c.fala({"op": "transacoes"})
    ok("a transacao aparece na lista", r["resultado"]["total"] >= 1,
       json.dumps(r)[:200])
    d.morrer()
    # A saida da conexao e assincrona; espera com PRAZO em vez de dormir fixo.
    fim, sumiu = time.time() + 10, False
    while time.time() < fim:
        if c.fala({"op": "transacoes"})["resultado"]["total"] == 0:
            sumiu = True
            break
        time.sleep(0.2)
    ok("a queda da conexao desfez a transacao", sumiu)
    ok("e nada foi gravado", quantas(c, "pedidos") == 0)

    print("\n== 6. o SQL, com as clausulas declaradas ==")
    r = c.fala({"op": "sql", "database": "loja",
                "texto": "BEGIN TRANSACTION SCOPE (clientes, pedidos) "
                         "TIMEOUT 5s LOCK TIMEOUT 500ms LOCK MODE AUTO"})
    dentro = r["resultado"]["resultado"] if r.get("ok") else {}
    ok("BEGIN pelo SQL abre", dentro.get("transaction_state") == "ACTIVE",
       json.dumps(r)[:250])
    ok("o LOCK TIMEOUT declarado chegou",
       dentro.get("lock_timeout_ms") == 500, json.dumps(dentro)[:200])
    ok("o escopo efetivo tem as duas tabelas",
       len(dentro.get("tabelas_efetivas", [])) == 2, json.dumps(dentro)[:250])
    c.fala({"op": "inserir", "database": "loja", "tabela": "pedidos",
            "linha": {"id": 7, "nome": "pelo sql"}})
    r = c.fala({"op": "sql", "texto": "COMMIT"})
    ok("COMMIT pelo SQL confirma",
       r.get("ok") and r["resultado"]["resultado"]["transaction_state"] == "COMMITTED",
       json.dumps(r)[:200])
    ok("e a linha esta la", quantas(c, "pedidos") == 1)

    print("\n== 6b. o DIARIO: o rollback nao produz evento nenhum ==")
    # E a §6.3 do desenho, e ela e a peca que faz a replicacao nao precisar
    # mudar: uma transacao revertida nao chega aplicada na replica porque ela
    # NAO CHEGA. Nada foi gravado, logo nada foi journalizado, logo nao ha o
    # que servir -- e nao ha supressao nenhuma a implementar do outro lado.
    def eventos(c, tabela):
        # `tabelas` e um OBJETO por nome de tabela, e nao uma lista.
        r = c.fala({"op": "posicao", "database": "loja"})
        return r["resultado"]["tabelas"].get(tabela, {}).get("eventos", -1)

    antes_dos_eventos = eventos(c, "clientes")
    f = Ligacao()
    f.fala({"op": "begin"})
    for i in range(500, 600):
        f.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": i, "nome": f"revertido {i}"}})
    ok("o diario NAO anda com a transacao aberta",
       eventos(c, "clientes") == antes_dos_eventos,
       f"{antes_dos_eventos} -> {eventos(c, 'clientes')}")
    f.fala({"op": "rollback"})
    ok("e continua parado depois do ROLLBACK",
       eventos(c, "clientes") == antes_dos_eventos,
       f"{antes_dos_eventos} -> {eventos(c, 'clientes')}")

    # E o COMMIT produz os eventos na ordem, de uma vez -- indistinguiveis de
    # um `inserir_lote` para quem os aplica. Uma replica de QUALQUER versao,
    # inclusive anterior a esta rodada, aplica sem saber que houve transacao.
    f.fala({"op": "begin"})
    for i in range(600, 640):
        f.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": i, "nome": f"confirmado {i}"}})
    f.fala({"op": "commit"})
    ok("o COMMIT produz um evento por linha, e nem um a mais",
       eventos(c, "clientes") == antes_dos_eventos + 40,
       f"{antes_dos_eventos} -> {eventos(c, 'clientes')}, esperava +40")

    # A prova de que o `.log` nao ganhou operacao nova: toda tag continua
    # sendo 1, 2 ou 3. Uma tag `BEGIN` faria `Operacao::de_tag` devolver
    # Corrompido numa replica antiga -- ela PARARIA, em vez de ignorar.
    r = c.fala({"op": "diario", "database": "loja", "tabela": "clientes",
                "limite": 200})
    ops = {e["operacao"] for e in r["resultado"]["eventos"]} if r.get("ok") else set()
    ok("o diario nao ganhou operacao nova",
       bool(ops) and ops <= {"inclusao", "alteracao", "exclusao"}, f"{sorted(ops)}")
    f.morrer()

    print("\n== 7. SIGKILL no meio de um COMMIT ==")
    # A marca `.tx` e escrita e sincronizada ANTES da passada. Para matar o
    # processo com ela no disco, o commit tem de ser grande o bastante para a
    # passada durar -- e o SIGKILL sai de outra thread, no meio.
    e = Ligacao()
    e.fala({"op": "begin"})
    for i in range(1000, 4000):
        e.fala({"op": "inserir", "database": "loja", "tabela": "pedidos",
                "linha": {"id": i, "nome": f"p{i}"}})
    antes_do_commit = quantas(c, "pedidos")
    pid = PIDS[-1]

    import threading
    morto = threading.Event()

    def matar():
        # Espera a marca aparecer no disco: e ela que prova que o commit
        # COMECOU. Matar antes dela seria matar uma transacao que nunca
        # comecou a confirmar, e o teste nao provaria nada.
        fim = time.time() + 15
        while time.time() < fim:
            if marcas():
                os.kill(pid, signal.SIGKILL)
                morto.set()
                return
            time.sleep(0.002)

    t = threading.Thread(target=matar)
    t.start()
    try:
        e.fala({"op": "commit"})
    except (OSError, ValueError, json.JSONDecodeError):
        pass  # o servidor morreu no meio da resposta, que e o esperado
    t.join(timeout=20)
    ok("o processo foi morto com a marca no disco", morto.is_set())
    esperar_porta_fechar(PORTA)

    sobrou = marcas()
    ok("a marca .tx ficou no disco", len(sobrou) >= 1, f"{sobrou}")

    print("\n== 8. o banco reabre e SABE DIZER o que aconteceu ==")
    subir()
    ok("o servidor voltou", esperar_porta(PORTA))
    g = Ligacao()
    total = quantas(g, "pedidos")
    with open(os.path.join(BASE, "servidor.log")) as f:
        log = f.read()
    ok("o relatorio de recuperacao saiu no arranque",
       "PHXSQL Recovery" in log,
       log[-400:].replace("\n", " | ")[:300])
    ok("e ele nao inventa linha que nao mede",
       "Pages redone" not in log and "paginas refeitas" not in log)

    # ------------------------------------------------------------------
    # O QUE SE AFIRMA AQUI E O CONTRATO, E NAO UM DOS DOIS DESFECHOS.
    #
    # A primeira versao deste passo exigia sempre as 3.000 linhas, e ela era
    # instavel POR CONSTRUCAO: matar o processo no instante certo e uma
    # corrida, e os dois desfechos sao LEGITIMOS. O SIGKILL pode cair antes
    # de o `fsync` da marca terminar -- e ai a marca fica truncada, o CRC nao
    # confere, e isso e um commit que NUNCA COMECOU.
    #
    # A pergunta do contrato nao e «as 3.000 estao la?». E:
    #
    #     depois de reiniciar, o banco consegue determinar de forma
    #     INEQUIVOCA se esta transacao foi COMMITTED ou ABORTED?
    #
    # Entao o que se exige e: nunca METADE. Ou 3.000, ou nenhuma -- e o
    # relatorio diz qual das duas, sem o teste ter de adivinhar.
    # ------------------------------------------------------------------
    completou = "transacoes completadas ........ 1" in log
    descartou = "marcas ilegiveis descartadas .. 1" in log
    ok("o relatorio diz UMA das duas, e nao as duas nem nenhuma",
       completou != descartou,
       f"completadas={completou} descartadas={descartou}")

    if completou:
        ok("COMMITTED: as 3.000 linhas estao la",
           total == antes_do_commit + 3000,
           f"{total} (antes {antes_do_commit})")
    else:
        ok("ABORTED: nenhuma das 3.000 entrou -- a marca nem chegou inteira",
           total == antes_do_commit,
           f"{total} (antes {antes_do_commit})")

    # E o que NUNCA pode acontecer, dito como asserção propria: metade.
    # O detalhe conta o FATO, e nao a negacao: o `ok()` imprime esta string
    # tambem quando passa, e «3001 nao e nem 1 nem 3001» ao lado de um OK faz
    # quem le a bateria parar e desconfiar de um teste que esta certo.
    ok("em nenhum caso a transacao ficou pela METADE",
       total in (antes_do_commit, antes_do_commit + 3000),
       f"{total}, e os unicos desfechos validos eram "
       f"{antes_do_commit} ou {antes_do_commit + 3000}")
    ok("a marca sumiu depois da recuperacao", not marcas(), f"{marcas()}")

    # Reabrir de novo nao pode duplicar nada -- a recuperacao e idempotente.
    g.morrer()
    for p in list(PIDS):
        try:
            os.kill(p, signal.SIGTERM)
        except ProcessLookupError:
            pass
    esperar_porta_fechar(PORTA)
    subir()
    esperar_porta(PORTA)
    h = Ligacao()
    de_novo = quantas(h, "pedidos")
    ok("reabrir de novo nao duplica nem perde", de_novo == total,
       f"{de_novo}, o mesmo de antes ({total})")

    print("\n== 9. o comportamento velho continua velho depois de tudo ==")
    r = h.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": 9999, "nome": "sem transacao"}})
    ok("insercao solta continua gravando na hora", r.get("ok"),
       json.dumps(r)[:160])
    ok("e o estado desta conexao e IDLE",
       h.fala({"op": "transacao"})["resultado"]["transaction_state"] == "IDLE")

    derrubar()
    print()
    if FALHAS:
        print(f"{len(FALHAS)} FALHA(S):")
        for f in FALHAS:
            print(f"  - {f}")
        sys.exit(1)
    print("tudo verde.")


if __name__ == "__main__":
    try:
        main()
    finally:
        derrubar()
