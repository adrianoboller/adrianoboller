#!/usr/bin/env python3
"""MULTILINK: uma consulta que atravessa MAIS DE UM servidor.

    python3 bancada/conexoes/multilink.py

DBLINK liga UM PhxSql a UM banco de fora. "MULTILINK", no sentido em que o
dono pediu (o nome vem do pacote proprietario analisado em
`docs/MULTILINK.md`, e do HFSQL(R) de onde ele saiu) e o HUB: um servidor com
VARIAS ligacoes, de onde uma unica sessao do cliente le dado que mora em mais
de uma maquina.

Sobe TRES `phxsqld` (faixa 6400-6419 desta frente):

    hub (6412) ---dblink "vendas"---> phx-vendas (6413)
        \\_______dblink "estoque"---> phx-estoque (6414)

O cliente fala SO com o hub. Cada resposta que o hub da PELO DBLINK e
conferida contra a mesma pergunta feita DIRETO a cada servidor de origem --
dois oraculos independentes, a mesma regra da bancada `dblink/prova-phxsql.py`.

Nunca usa `pkill`: mata os tres pelo PID guardado.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
PHXSQLD = RAIZ / "target/release/phxsqld"
TRABALHO = Path(f"/tmp/phx-f4-{os.getpid()}-multilink")

PORTA_HUB, PORTA_VENDAS, PORTA_ESTOQUE = 6412, 6413, 6414
TOKEN_HUB, TOKEN_VENDAS, TOKEN_ESTOQUE = "t-hub", "t-vendas", "t-estoque"
SENHA_HUB, SENHA_VENDAS, SENHA_ESTOQUE = "senha-hub-1", "senha-vendas-2", "senha-estoque-3"

falhas = []


def afirma(rotulo, condicao, visto=""):
    ok = bool(condicao)
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}" + (f"  -- {visto}" if visto != "" else ""))
    if not ok:
        falhas.append(rotulo)


def confere(rotulo, pelo_hub, direto):
    ok = pelo_hub == direto
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}")
    print(f"        pelo hub: {pelo_hub}")
    print(f"        direto:   {direto}")
    if not ok:
        falhas.append(rotulo)


def hash_da_senha(senha):
    r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


class Phxsqld:
    def __init__(self, nome, porta, token, senha):
        self.nome, self.porta, self.token = nome, porta, token
        self.base = TRABALHO / nome
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        (self.base / "config.json").write_text(json.dumps({
            "base": "dados",
            "bind": f"127.0.0.1:{porta}",
            "token": token,
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": hash_da_senha(senha)},
            "usuarios": [],
        }, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
        )
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", porta), timeout=2).close()
                print(f"{nome}: phxsqld pid {self.proc.pid} na porta {porta}")
                return
            except OSError:
                pass
        raise SystemExit(f"o {nome} nao subiu; veja {self.base / 'servidor.log'}")

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Cliente:
    def __init__(self, servidor, senha):
        self.token = servidor.token
        self.s = socket.create_connection(("127.0.0.1", servidor.porta), timeout=30)
        self.f = self.s.makefile("rwb")
        self.call({"op": "login", "usuario": "root", "senha": senha})

    def bruto(self, d):
        d.setdefault("token", self.token)
        self.s.sendall((json.dumps(d) + "\n").encode())
        return json.loads(self.f.readline())

    def call(self, d):
        r = self.bruto(d)
        if not r.get("ok"):
            raise SystemExit(f"{d['op']} recusado: " + json.dumps(r)[:400])
        return r.get("resultado")


def principal():
    if not PHXSQLD.exists():
        sys.exit(f"nao achei {PHXSQLD}. Rode `cargo build --release` antes.")

    print("=== MULTILINK: um HUB, dois servidores de origem, uma sessao de cliente ===\n")
    hub = Phxsqld("hub", PORTA_HUB, TOKEN_HUB, SENHA_HUB)
    vendas = Phxsqld("phx-vendas", PORTA_VENDAS, TOKEN_VENDAS, SENHA_VENDAS)
    estoque = Phxsqld("phx-estoque", PORTA_ESTOQUE, TOKEN_ESTOQUE, SENHA_ESTOQUE)
    try:
        c_hub = Cliente(hub, SENHA_HUB)
        c_vendas = Cliente(vendas, SENHA_VENDAS)
        c_estoque = Cliente(estoque, SENHA_ESTOQUE)

        # --- os dois servidores de origem, cada um com o SEU dado, que o
        #     hub nunca importa -- ele so LE de fora.
        c_vendas.call({"op": "criar_database", "database": "com"})
        c_vendas.call({"op": "criar_tabela", "database": "com", "tabela": "pedidos", "colunas": [
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "cliente", "tipo": "Str(30)", "obrigatoria": True},
            {"nome": "total", "tipo": "Decimal(12,2)", "obrigatoria": True},
        ], "indices": [{"nome": "pk_id", "colunas": ["id"], "unico": True, "primario": True}]})
        c_vendas.call({"op": "inserir_lote", "database": "com", "tabela": "pedidos", "linhas": [
            [1, "Ana Maria", "1250.00"], [2, "Carlos", "780.50"], [3, "Bete", "3020.90"],
        ]})

        c_estoque.call({"op": "criar_database", "database": "log"})
        c_estoque.call({"op": "criar_tabela", "database": "log", "tabela": "itens", "colunas": [
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "sku", "tipo": "Str(20)", "obrigatoria": True},
            {"nome": "quantidade", "tipo": "Int4", "obrigatoria": True},
        ], "indices": [{"nome": "pk_id", "colunas": ["id"], "unico": True, "primario": True}]})
        c_estoque.call({"op": "inserir_lote", "database": "log", "tabela": "itens", "linhas": [
            [1, "SKU-001", 40], [2, "SKU-002", 7], [3, "SKU-003", 120],
        ]})

        # --- o hub cadastra as DUAS ligacoes ---
        print("\n-- 1. o hub cadastra DUAS ligacoes -- e' isto que faz um DBLINK virar MULTILINK")
        r = c_hub.call({"op": "dblink_salvar", "nome": "vendas", "motor": "phxsql",
                        "host": "127.0.0.1", "porta": PORTA_VENDAS, "token_remoto": TOKEN_VENDAS,
                        "usuario": "root", "senha": SENHA_VENDAS, "database": "com",
                        "somente_leitura": True})
        afirma("dblink_salvar 'vendas' -> phx-vendas", r.get("gravado"), r)
        r = c_hub.call({"op": "dblink_salvar", "nome": "estoque", "motor": "phxsql",
                        "host": "127.0.0.1", "porta": PORTA_ESTOQUE, "token_remoto": TOKEN_ESTOQUE,
                        "usuario": "root", "senha": SENHA_ESTOQUE, "database": "log",
                        "somente_leitura": True})
        afirma("dblink_salvar 'estoque' -> phx-estoque", r.get("gravado"), r)
        ligacoes = sorted(l["nome"] for l in c_hub.call({"op": "dblink"})["ligacoes"])
        afirma("o hub lista as duas ligacoes", ligacoes == ["estoque", "vendas"], ligacoes)

        print("\n-- 2. dblink_tabelas dos DOIS lados, na MESMA sessao do cliente com o hub")
        tv = c_hub.call({"op": "dblink_tabelas", "dblink": "vendas", "database": "com"})["tabelas"]
        te = c_hub.call({"op": "dblink_tabelas", "dblink": "estoque", "database": "log"})["tabelas"]
        confere("tabelas vistas atraves de 'vendas'", [t["nome"] for t in tv], ["pedidos"])
        confere("tabelas vistas atraves de 'estoque'", [t["nome"] for t in te], ["itens"])

        print("\n-- 3. UMA sessao de cliente com o hub, que ATRAVESSA os dois servidores")
        # O cliente nao abre soquete nenhum com phx-vendas ou phx-estoque:
        # so fala com o hub, que salta para os dois. SUM()/agregado nao
        # existe na op "sql" (so COUNT(*) sai do cabecalho em O(1) --
        # achado real desta rodada, e nao suposto: a primeira tentativa com
        # SUM(total) foi RECUSADA, com o motivo abaixo, e a prova usa o que
        # o motor de fato tem: ler as linhas e agregar do lado do cliente,
        # exatamente como uma tela faria.
        pedidos = c_hub.call({"op": "dblink_consultar", "dblink": "vendas",
                              "sql": "SELECT cliente, total FROM pedidos"})
        itens = c_hub.call({"op": "dblink_consultar", "dblink": "estoque",
                            "sql": "SELECT sku, quantidade FROM itens"})
        total_vendido = sum(float(l[1]) for l in pedidos["linhas"])
        total_em_estoque = sum(int(l[1]) for l in itens["linhas"])
        print(f"      relatorio combinado, pelo HUB, numa unica sessao de cliente:")
        print(f"        {len(pedidos['linhas'])} pedidos  (via dblink 'vendas',  phx-vendas :{PORTA_VENDAS}) somam  R$ {total_vendido:.2f}")
        print(f"        {len(itens['linhas'])} itens    (via dblink 'estoque', phx-estoque:{PORTA_ESTOQUE}) somam  {total_em_estoque} unidades")

        direto_v = c_vendas.call({"op": "sql", "database": "com",
                                  "texto": "SELECT cliente, total FROM pedidos"})
        direto_e = c_estoque.call({"op": "sql", "database": "log",
                                   "texto": "SELECT sku, quantidade FROM itens"})
        # dblink_consultar devolve LINHA COMO LISTA (posicional, na ordem das
        # `colunas`); a op "sql" local devolve LINHA COMO OBJETO. Formatos
        # diferentes de proposito (um fala com o SQL de fora, o outro com o
        # tradutor local) -- a comparacao real e por VALOR, apos igualar a
        # forma, exatamente como docs/DBLINK.md manda ler por nome, nunca
        # por posicao.
        confere("os pedidos, pelo hub contra o phx-vendas direto",
                [[str(l[0]), str(l[1])] for l in pedidos["linhas"]],
                [[l["cliente"], l["total"]] for l in direto_v["linhas"]])
        confere("os itens, pelo hub contra o phx-estoque direto",
                [[str(l[0]), str(l[1])] for l in itens["linhas"]],
                [[l["sku"], str(l["quantidade"])] for l in direto_e["linhas"]])

        r = c_hub.bruto({"op": "dblink_consultar", "dblink": "vendas",
                         "sql": "SELECT SUM(total) AS total FROM pedidos"})
        afirma("achado real desta rodada: SUM() nao existe na op sql -- so COUNT(*)",
               not r.get("ok") and "SUM() nao tem quem calcule" in r.get("erro", ""),
               r.get("erro"))

        print("\n-- 4. o hub continua SEM base propria de dado -- ele so LE de fora")
        afirma("o hub nao tem nenhum database dele", c_hub.call({"op": "bancos"}) == [],
               c_hub.call({"op": "bancos"}))

        print("\n-- 5. o limite real do MULTILINK aqui: nao ha SQL que atravesse os DOIS")
        # "SELECT ... FROM vendas.pedidos JOIN estoque.itens" nao existe --
        # o FROM so enderece bancos LOCAIS (docs/SQL.md). Cada dblink_consultar
        # roda inteiro DENTRO de um so servidor de origem; quem combina os
        # dois lados e o CLIENTE (ou a tela), como o passo 3 acima fez.
        r = c_hub.bruto({"op": "sql", "database": "qualquer",
                         "texto": "SELECT * FROM vendas.pedidos"})
        afirma("SQL nao enderega um DBLINK no FROM -- recusa (o hub nem tem database local)",
               not r.get("ok"), r.get("erro", json.dumps(r))[:200])
    finally:
        for s in (hub, vendas, estoque):
            s.parar()
        shutil.rmtree(TRABALHO, ignore_errors=True)

    print("\n" + ("=== TUDO OK ===" if not falhas else f"=== {len(falhas)} FALHA(S): {falhas} ==="))
    return 1 if falhas else 0


if __name__ == "__main__":
    raise SystemExit(principal())
