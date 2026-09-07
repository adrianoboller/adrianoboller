#!/usr/bin/env python3
"""Quanto CUSTA um quorum aqui, hoje, com o codigo que ja existe.

    python3 bancada/quorum/medir.py [voltas]

# A pergunta que este medidor existe para responder

O dono perguntou se da para replicar a gravacao por QUORUM, sem usar a
replicacao. A resposta de projeto e outro assunto; o que este medidor faz e
tirar do caminho a premissa que estava escondida num numero so.

A bancada de replicacao publica um atraso de **826 a 2014 ms** por operacao. E
facil ler isso como «o dado leva 826 ms para chegar na replica», e concluir
que um commit sincrono custaria isso. **Nao e o que aquele numero mede.**
Aquela bancada roda com `reconectar_em: 2`, e o laco da replica DORME esse
tempo quando nao acha nada. O atraso publicado e, quase todo, sono.

Entao aqui as tres coisas se medem SEPARADAS:

  gravar    -- o `inserir` no master, sozinho. E o que se paga hoje.
  levar     -- `replicar` no master + `aplicar` na replica, chamados NA HORA,
               sem sono nenhum. E o que um commit por quorum pagaria a mais,
               por replica.
  quorum    -- o mesmo, contra DUAS replicas, e o relogio parando quando a
               enesima confirma. Com 3 nos: 2-de-3 espera a PRIMEIRA replica
               (o master conta como um voto), 3-de-3 espera a ULTIMA.

# Por que as replicas sobem com `reconectar_em` gigante

Porque o laco delas competiria com a medicao: se a replica puxar sozinha no
instante errado, o `replicar` daqui acha zero evento e o numero sai menor do
que a verdade -- medidor que mede o proprio concorrente. Com uma hora de sono,
depois da primeira rodada elas ficam quietas e quem puxa e este script.

E ha um portao para isso: se o `aplicar` devolver zero evento aplicado, o
medidor PARA. Zero e o sintoma de a replica ja ter puxado por conta propria, e
publicar a media de zeros seria publicar que o quorum e de graca.
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
BASE = "/tmp/phx-quorum"

PORTA_MASTER = 5900
PORTAS_REPLICA = [5901, 5902]
TOKEN = "quorum"
USUARIO = "adm"
SENHA = "segredo1"
DB, TAB = "loja", "clientes"

PIDS = []
SOQUETES = []


def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    # Os nomes sao os do servidor, e nao os que eu supus: a primeira corrida
    # levou ACESSO_NEGADO com "escrever", que nao existe. Direito inventado da
    # recusa, e recusa nao lida vira ausencia publicada.
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def config_master(h):
    return {
        "bind": f"127.0.0.1:{PORTA_MASTER}", "base": "base", "token": TOKEN,
        "replicacao": {"papel": "source", "id_servidor": "master",
                       # Sem a imagem da linha a replica recebe so o rowid, e
                       # nao tem o que aplicar. Foi assim que uma medicao
                       # anterior desta casa "replicou" sem replicar nada.
                       "imagem_da_linha": True},
        "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                      "senha_hash": h, "bases": permissoes()}],
    }


def config_replica(h, n):
    return {
        "bind": f"127.0.0.1:{PORTAS_REPLICA[n]}", "base": "base", "token": TOKEN,
        "somente_leitura": True,
        "replicacao": {
            "papel": "replica", "id_servidor": f"r{n}", "imagem_da_linha": True,
            # Uma HORA de sono: o laco proprio nao pode competir com a medicao.
            "origens": [{"nome": "master", "host": "127.0.0.1",
                         "porta": PORTA_MASTER, "token": TOKEN,
                         "usuario": USUARIO, "senha_hash": h,
                         "databases": [DB], "reconectar_em": 3600}],
        },
        "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                      "senha_hash": h, "bases": permissoes()}],
    }


class Ligacao:
    def __init__(self, porta, prazo=60):
        self.porta, self.prazo = porta, prazo
        self._abrir()

    def _abrir(self):
        ultimo = None
        for _ in range(150):
            try:
                s = socket.create_connection(("127.0.0.1", self.porta), timeout=self.prazo)
                f = s.makefile("rwb")
                SOQUETES.append((s, f))
                self.s, self.f = s, f
                r = self._cru({"op": "login", "usuario": USUARIO, "senha": SENHA})
                if not r.get("ok"):
                    raise SystemExit(f"login na porta {self.porta}: {r}")
                return
            except OSError as e:
                ultimo = e
                time.sleep(0.2)
        raise SystemExit(f"porta {self.porta} nunca respondeu: {ultimo}")

    def _cru(self, pedido):
        pedido.setdefault("token", TOKEN)
        self.f.write((json.dumps(pedido) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise ConnectionError("o servidor fechou a conexao")
        return json.loads(linha.decode())

    def __call__(self, pedido):
        try:
            return self._cru(dict(pedido))
        except (OSError, ValueError):
            self._abrir()
            return self._cru(dict(pedido))


def subir(dir_, porta):
    os.makedirs(os.path.join(BASE, dir_), exist_ok=True)
    log = open(os.path.join(BASE, dir_, "servidor.log"), "w")
    p = subprocess.Popen([PHXSQLD], cwd=os.path.join(BASE, dir_),
                         stdout=log, stderr=subprocess.STDOUT)
    PIDS.append(p.pid)
    return p


def derrubar():
    """Mata SO os PIDs que este script subiu. Nunca por nome."""
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    for s, f in SOQUETES:
        try:
            f.close(); s.close()
        except OSError:
            pass


def corpo(r):
    return r.get("resultado") or {}


def main():
    voltas = int(sys.argv[1]) if len(sys.argv) > 1 else 60
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode cargo build --release")
    subprocess.run(["rm", "-rf", BASE], check=False)
    h = hash_da_senha(SENHA)
    for d, cfg in [("master", config_master(h)),
                   ("r0", config_replica(h, 0)),
                   ("r1", config_replica(h, 1))]:
        os.makedirs(os.path.join(BASE, d), exist_ok=True)
        with open(os.path.join(BASE, d, "config.json"), "w") as f:
            json.dump(cfg, f, indent=2)
    # A ORDEM IMPORTA, e ela custou uma corrida: com `reconectar_em` de uma
    # hora, a replica faz UMA rodada ao subir e depois dorme. Subindo antes de
    # o database existir, essa unica rodada nao acha nada, e o `aplicar` da
    # primeira volta erra com "database loja nao existe". O esquema nasce
    # PRIMEIRO; so entao as replicas sobem e a rodada delas o alcanca.
    subir("master", PORTA_MASTER)
    m = Ligacao(PORTA_MASTER)

    r = m({"op": "criar_database", "database": DB})
    if not r.get("ok") and "existe" not in str(r.get("erro", "")):
        sys.exit(f"MESA NAO POSTA: criar_database -- {r}")
    r = m({"op": "criar_tabela", "database": DB, "tabela": TAB,
           "colunas": [{"nome": "id", "tipo": "Int8"},
                       {"nome": "nome", "tipo": "Str(40)"}],
           "indices": [{"nome": "pk", "colunas": ["id"], "unico": True}]})
    if not r.get("ok"):
        sys.exit(f"MESA NAO POSTA: criar_tabela -- {r}")

    # Agora sim as replicas: a unica rodada delas alcanca o esquema, e depois
    # elas dormem uma hora e saem do caminho da medicao.
    for i, _ in enumerate(PORTAS_REPLICA):
        subir(f"r{i}", PORTAS_REPLICA[i])
    reps = [Ligacao(p) for p in PORTAS_REPLICA]
    time.sleep(4)

    # PORTAO: a replica tem de ter alcancado o esquema sozinha. Sem isto, o
    # medidor mediria o custo de aplicar em tabela que nao existe -- que erra,
    # mas erraria RAPIDO, e um numero pequeno por engano e o pior dos numeros.
    for i, rep in enumerate(reps):
        r = rep({"op": "tabelas", "database": DB})
        nomes = [x.get("nome", x) if isinstance(x, dict) else x
                 for x in (corpo(r).get("tabelas") or [])]
        if TAB not in nomes:
            sys.exit(f"MESA NAO POSTA: a replica {i} nao alcancou {DB}.{TAB} -- {r}")

    gravar, levar, dois_de_tres, tres_de_tres = [], [], [], []
    posicao = [0, 0]
    for v in range(voltas):
        t0 = time.perf_counter()
        r = m({"op": "inserir", "database": DB, "tabela": TAB,
               "valores": {"id": 10_000 + v, "nome": f"n{v}"}})
        t1 = time.perf_counter()
        if not r.get("ok"):
            sys.exit(f"inserir falhou na volta {v}: {r}")
        gravar.append((t1 - t0) * 1000)

        # O QUORUM, cronometrado: puxar do master e aplicar em cada replica,
        # NA HORA. O relogio de cada replica comeca junto, depois do commit.
        marcas = []
        for i, rep in enumerate(reps):
            ta = time.perf_counter()
            ev = m({"op": "replicar", "database": DB, "tabela": TAB,
                    "desde": posicao[i], "max": 500})
            eventos = corpo(ev).get("eventos") or []
            if not eventos:
                sys.exit(
                    f"ZERO EVENTOS na volta {v}, replica {i}: a replica puxou "
                    "sozinha e este medidor mediria o proprio concorrente. "
                    "Confira o `reconectar_em` de 3600 no config."
                )
            ap = rep({"op": "aplicar", "database": DB, "tabela": TAB,
                      "eventos": eventos})
            if not ap.get("ok"):
                sys.exit(f"aplicar falhou na volta {v}, replica {i}: {ap}")
            tb = time.perf_counter()
            posicao[i] = corpo(ev).get("ate", posicao[i])
            marcas.append((tb - ta) * 1000)
        levar.extend(marcas)
        # 3 nos: o master ja e um voto. 2-de-3 espera a PRIMEIRA replica a
        # confirmar; 3-de-3 espera a ULTIMA.
        dois_de_tres.append(min(marcas))
        tres_de_tres.append(max(marcas))

    def med(x):
        return round(statistics.median(x), 3)

    def faixa(x):
        return [round(min(x), 3), round(max(x), 3)]

    saida = {
        "voltas": voltas,
        "gravar_ms": med(gravar), "gravar_faixa": faixa(gravar),
        "levar_ms": med(levar), "levar_faixa": faixa(levar),
        "quorum_2de3_ms": med(dois_de_tres), "quorum_2de3_faixa": faixa(dois_de_tres),
        "quorum_3de3_ms": med(tres_de_tres), "quorum_3de3_faixa": faixa(tres_de_tres),
        "commit_hoje_ms": med(gravar),
        "commit_com_2de3_ms": round(med(gravar) + med(dois_de_tres), 3),
        "commit_com_3de3_ms": round(med(gravar) + med(tres_de_tres), 3),
        "vezes_2de3": round((med(gravar) + med(dois_de_tres)) / med(gravar), 2),
        "vezes_3de3": round((med(gravar) + med(tres_de_tres)) / med(gravar), 2),
        "maquina": "tres processos phxsqld em 127.0.0.1, no mesmo container",
        "aviso_da_maquina": "TUDO em localhost: a rede real custa mais, e este "
                            "numero e o PISO do que um quorum custaria",
        "medido_em": time.strftime("%Y-%m-%d %H:%M"),
    }
    with open(os.path.join(AQUI, "resultados.json"), "w", encoding="utf-8") as f:
        json.dump(saida, f, ensure_ascii=False, indent=1)

    print(f"\n{voltas} voltas, tres servidores em 127.0.0.1\n")
    print(f"  gravar no master (o que se paga HOJE)  {saida['gravar_ms']:8.3f} ms"
          f"   faixa {saida['gravar_faixa']}")
    print(f"  levar para UMA replica, na hora        {saida['levar_ms']:8.3f} ms"
          f"   faixa {saida['levar_faixa']}")
    print()
    print(f"  commit hoje (sem esperar ninguem)      {saida['commit_hoje_ms']:8.3f} ms")
    print(f"  commit esperando 2-de-3                {saida['commit_com_2de3_ms']:8.3f} ms"
          f"   {saida['vezes_2de3']}x")
    print(f"  commit esperando 3-de-3                {saida['commit_com_3de3_ms']:8.3f} ms"
          f"   {saida['vezes_3de3']}x")
    print(f"\nresultado gravado: {os.path.join(AQUI, 'resultados.json')}")


if __name__ == "__main__":
    try:
        main()
    finally:
        derrubar()
