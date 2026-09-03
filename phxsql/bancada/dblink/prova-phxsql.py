#!/usr/bin/env python3
"""A prova do DbLink de um PhxSql para OUTRO PhxSql, por soquete.

    python3 bancada/dblink/prova-phxsql.py

O pedido 166 dizia, medido, que este caminho NAO existia: `{"motor":"phxsql"}`
respondia `motor de dblink desconhecido`. Esta prova e o que fecha o pedido --
e o que impede a mesma frase de voltar a ser verdade sem ninguem ver.

Por que soquete, e nao teste de unidade
---------------------------------------
Porque o que se prova aqui e a CONVERSA: dois processos, duas portas, dois
portoes de credencial em serie (o token de servico e o login) e um cliente que
so existe de verdade quando ha um servidor do outro lado. Teste unitario nao
prova queda de conexao nem token errado -- ja custou uma rodada nesta casa.

O oraculo
---------
Cada resposta que o `phx-a` da PELO DBLINK e conferida contra a mesma pergunta
feita DIRETAMENTE ao `phx-b`, na porta dele. Dois caminhos independentes tem de
dizer a mesma coisa; conferir contra o que este script acha provaria so que o
script e o servidor concordam.

O que ela sobe e o que ela nao mata
-----------------------------------
Dois `phxsqld` proprios, em portas proprias, mortos pelo PID -- nunca `pkill`,
que derrubaria o servidor de outra frente na mesma maquina.
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
PHXSQLD = Path(os.environ.get("PHX_PHXSQLD", RAIZ / "target/release/phxsqld"))
TRABALHO = Path(os.environ.get("PHX_TRABALHO", f"/tmp/phx-dblink-phx-{os.getpid()}"))

PORTA_A = int(os.environ.get("PORTA_A", "7491"))
PORTA_B = int(os.environ.get("PORTA_B", "7492"))
TOKEN_A, TOKEN_B = "prova-phx-a", "prova-phx-b"
SENHA_A, SENHA_B = "prova-a-9876", "prova-b-5432"

falhas = []


def hash_da_senha(senha):
    r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


class Phxsqld:
    """Morre pelo PID, e so ele."""

    def __init__(self, nome, porta, token, senha):
        self.porta, self.token, self.senha = porta, token, senha
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
                return
            except OSError:
                pass
        raise SystemExit(f"o {nome} nao subiu; veja {self.base / 'servidor.log'}")

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Cliente:
    def __init__(self, servidor):
        self.token = servidor.token
        self.s = socket.create_connection(("127.0.0.1", servidor.porta), timeout=60)
        self.f = self.s.makefile("rwb")
        self.call({"op": "login", "usuario": "root", "senha": servidor.senha})

    def bruto(self, d):
        d.setdefault("token", self.token)
        self.s.sendall((json.dumps(d) + "\n").encode())
        return json.loads(self.f.readline())

    def call(self, d):
        r = self.bruto(d)
        if not r.get("ok"):
            raise SystemExit(f"{d['op']} recusado: " + json.dumps(r)[:400])
        return r.get("resultado")


def confere(rotulo, pelo_dblink, direto):
    """O que o phx-a viu PELO DBLINK contra o que o phx-b responde na porta
    dele. Nunca contra o que este script acha."""
    ok = pelo_dblink == direto
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}")
    print(f"        dblink: {pelo_dblink}")
    print(f"        direto: {direto}")
    if not ok:
        falhas.append(rotulo)


def afirma(rotulo, condicao, visto):
    ok = bool(condicao)
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto}")
    if not ok:
        falhas.append(rotulo)


def principal():
    if not PHXSQLD.exists():
        raise SystemExit(f"nao achei {PHXSQLD}.\nRode `cargo build --release` antes.")

    print("=== DbLink de um PhxSql para OUTRO PhxSql ===")
    a = Phxsqld("phx-a", PORTA_A, TOKEN_A, SENHA_A)
    b = Phxsqld("phx-b", PORTA_B, TOKEN_B, SENHA_B)
    try:
        ca, cb = Cliente(a), Cliente(b)

        # --- o dado mora no phx-b, e o phx-a tem uma base propria que nao se
        #     mistura: e ela que prova que o DbLink LE de fora, nao importa.
        cb.call({"op": "criar_database", "database": "rh"})
        cb.call({"op": "criar_tabela", "database": "rh", "tabela": "funcionarios",
                 "colunas": [
                     {"nome": "id", "tipo": "Int8", "obrigatoria": True},
                     {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                     {"nome": "cidade", "tipo": "Str(30)"},
                     {"nome": "salario", "tipo": "Decimal(12,2)"},
                     {"nome": "ativo", "tipo": "Bool"}],
                 "indices": [{"nome": "pk_id", "colunas": ["id"],
                              "unico": True, "primario": True},
                             {"nome": "porCidade", "colunas": ["cidade"]}]})
        linhas = [[i, f"Pessoa {i:03}", "Blumenau" if i % 2 else "Joinville",
                   f"{1000 + i * 7}.50", i % 3 != 0] for i in range(1, 26)]
        cb.call({"op": "inserir_lote", "database": "rh",
                 "tabela": "funcionarios", "linhas": linhas})
        ca.call({"op": "criar_database", "database": "loja"})

        print("\n-- 0. o motor phxsql existe (era o que o pedido 166 media)")
        r = ca.bruto({"op": "dblink_salvar", "nome": "b", "motor": "phxsql",
                      "host": "127.0.0.1", "porta": PORTA_B,
                      "token_remoto": TOKEN_B, "usuario": "root", "senha": SENHA_B,
                      "database": "rh", "somente_leitura": True})
        afirma("dblink_salvar aceita motor phxsql", r.get("ok"),
               json.dumps(r.get("resultado", r))[:160])
        ficha = r.get("resultado", {}).get("ligacao", {})
        afirma("a senha nao volta na ficha", ficha.get("senha") == "(oculta)",
               ficha.get("senha"))
        afirma("o token nao volta na ficha",
               ficha.get("token_remoto") == "(oculto)", ficha.get("token_remoto"))
        # O caminho da resposta e relativo ao diretorio do SERVIDOR, e nao ao
        # deste script -- o `phxsqld` roda com `cwd` na base dele.
        arquivo = a.base / ca.call({"op": "dblink"})["arquivo"]
        afirma("o dblink.json fica so do dono (0600)",
               oct(os.stat(arquivo).st_mode & 0o777) == "0o600",
               oct(os.stat(arquivo).st_mode & 0o777))
        afirma("e o token do outro servidor NAO esta na resposta do protocolo",
               TOKEN_B not in json.dumps(ca.call({"op": "dblink"})),
               "so na ficha em disco")

        print("\n-- 1. dblink_testar: os DOIS portoes em serie, token e login")
        t = ca.call({"op": "dblink_testar", "dblink": "b"})
        confere("a versao do outro lado", t.get("versao"),
                cb.call({"op": "ping"})["phxsql"])
        confere("com quem o outro lado acha que fala", t.get("usuario_efetivo"),
                cb.call({"op": "quem_sou"})["login"])
        afirma("o papel dele veio junto", t.get("papel") == "isolado", t.get("papel"))

        print("\n-- 2. dblink_bancos contra o `bancos` do proprio phx-b")
        confere("as bases", sorted(ca.call({"op": "dblink_bancos", "dblink": "b"})["bancos"]),
                sorted(cb.call({"op": "bancos"})))
        afirma("o phx-a NAO importou nada: as bases dele continuam as dele",
               ca.call({"op": "bancos"}) == ["loja"], ca.call({"op": "bancos"}))

        print("\n-- 3. dblink_tabelas contra o `sistabelas` do phx-b")
        tabs = ca.call({"op": "dblink_tabelas", "dblink": "b", "database": "rh"})["tabelas"]
        sis = cb.call({"op": "sistabelas", "database": "rh"})["tabelas"][0]
        confere("os nomes", [t["nome"] for t in tabs],
                [t["tabela"] for t in cb.call({"op": "sistabelas", "database": "rh"})["tabelas"]])
        confere("a contagem de registros", tabs[0]["registros"], sis["registros"])
        confere("a chave primaria", tabs[0]["chave_primaria"], sis["chave_primaria"])
        confere("os bytes do .reg", tabs[0]["bytes"], sis["slots"] * sis["bytes_por_linha"])
        afirma("e a resposta DIZ que o numero e so o .reg",
               tabs[0]["bytes_de"] == ".reg", tabs[0]["bytes_de"])

        print("\n-- 4. dblink_estrutura: os nomes do SHOW, contra o `esquema` do phx-b")
        est = ca.call({"op": "dblink_estrutura", "dblink": "b",
                       "database": "rh", "tabela": "funcionarios"})
        col = est["colunas"]
        nomes = [c["nome"] for c in col["colunas"]]
        afirma("os seis primeiros nomes sao os do SHOW FULL COLUMNS",
               nomes[:6] == ["Field", "Type", "Null", "Key", "Default", "Comment"],
               nomes[:6])
        # A leitura e POR NOME, como o docs/DBLINK.md manda -- nunca por posicao.
        pos = {n: i for i, n in enumerate(nomes)}
        esq = cb.call({"op": "esquema", "database": "rh", "tabela": "funcionarios"})
        confere("as colunas, na ordem",
                [l[pos["Field"]] for l in col["linhas"]],
                [c["nome"] for c in esq["colunas"]])
        confere("os tipos", [l[pos["Type"]] for l in col["linhas"]],
                [c["tipo"] for c in esq["colunas"]])
        confere("quem aceita nulo",
                [l[pos["Null"]] for l in col["linhas"]],
                ["YES" if c["nullable"] else "NO" for c in esq["colunas"]])
        confere("qual e a chave primaria",
                [l[pos["Field"]] for l in col["linhas"] if l[pos["Key"]] == "PRI"],
                [c["nome"] for c in esq["colunas"] if c["primaria"]])

        idx = est["indices"]
        ipos = {c["nome"]: i for i, c in enumerate(idx["colunas"])}
        unicos = {l[ipos["Key_name"]]: l[ipos["Non_unique"]] for l in idx["linhas"]}
        confere("os indices",
                sorted(unicos), sorted(i["nome"] for i in esq["indices"]))
        afirma("Non_unique tem a polaridade do NOME: 0 e unico",
               unicos["pk_id"] == "0" and unicos["porCidade"] == "1", unicos)

        print("\n-- 5. dblink_ler contra o `varrer` do phx-b")
        pag = ca.call({"op": "dblink_ler", "dblink": "b", "database": "rh",
                       "tabela": "funcionarios", "limite": 10})
        lidos = {c["nome"]: i for i, c in enumerate(pag["colunas"])}
        direto = cb.call({"op": "varrer", "database": "rh",
                          "tabela": "funcionarios", "max": 10})["linhas"]
        confere("os nomes", [str(l[lidos["nome"]]) for l in pag["linhas"]],
                [l["nome"] for l in direto])
        confere("os decimais, sem perder casa",
                [l[lidos["salario"]] for l in pag["linhas"]],
                [l["salario"] for l in direto])
        # A armadilha que o `t`/`f` do PostgreSQL(R) armou uma vez: o booleano
        # sai `1`/`0`, e nao `true`, para toda comparacao `== "1"` valer.
        confere("o booleano, como 1/0",
                [l[lidos["ativo"]] for l in pag["linhas"]],
                ["1" if l["ativo"] else "0" for l in direto])
        afirma("a contagem do outro lado veio junta", pag["registros"] == 25,
               pag["registros"])
        afirma("e a resposta diz que ha mais pagina", pag["tem_mais"] is True,
               pag["tem_mais"])

        segunda = ca.call({"op": "dblink_ler", "dblink": "b", "database": "rh",
                           "tabela": "funcionarios", "limite": 10, "salto": 20})
        confere("a ultima pagina",
                [l[lidos["nome"]] for l in segunda["linhas"]],
                [l["nome"] for l in cb.call({"op": "varrer", "database": "rh",
                                             "tabela": "funcionarios",
                                             "max": 10, "pular": 20})["linhas"]])
        afirma("e ela diz que acabou", segunda["tem_mais"] is False, segunda["tem_mais"])

        print("\n-- 6. dblink_consultar: o SQL roda LA, no motor do phx-b")
        q = ca.call({"op": "dblink_consultar", "dblink": "b",
                     "sql": "SELECT nome, cidade FROM funcionarios WHERE cidade = 'Blumenau'"})
        la = cb.call({"op": "sql", "database": "rh",
                      "texto": "SELECT nome, cidade FROM funcionarios WHERE cidade = 'Blumenau'"})
        confere("a projecao", [c["nome"] for c in q["colunas"]], la["colunas"])
        confere("as linhas", [l[0] for l in q["linhas"]], [l["nome"] for l in la["linhas"]])
        cont = ca.call({"op": "dblink_consultar", "dblink": "b",
                        "sql": "SELECT COUNT(*) FROM funcionarios"})
        afirma("COUNT(*) devolve a contagem e NENHUMA linha de dado",
               len(cont["linhas"]) == 1 and cont["linhas"][0][0] == "25"
               and [c["nome"] for c in cont["colunas"]] == ["contagem"],
               json.dumps(cont)[:180])

        print("\n-- 7. o que ele RECUSA, e recusa dizendo por que")
        r = ca.bruto({"op": "dblink_ler", "dblink": "b", "database": "rh",
                      "tabela": "funcionarios", "ordem": "nome"})
        afirma("`ordem` recusa em vez de devolver a ordem errada calada",
               not r.get("ok") and "ordem de digitacao" in r.get("erro", ""),
               r.get("erro", json.dumps(r))[:170])
        r = ca.bruto({"op": "dblink_ligar", "dblink": "b",
                      "tabelas": [{"remota": "funcionarios", "local_database": "loja",
                                   "local_tabela": "func"}]})
        afirma("a sincronia recusa e manda para a REPLICACAO",
               not r.get("ok") and "REPLICACAO" in r.get("erro", ""),
               r.get("erro", json.dumps(r))[:200])
        r = ca.bruto({"op": "dblink_consultar", "dblink": "b",
                      "sql": "DELETE FROM funcionarios"})
        afirma("a ligacao somente-leitura recusa a escrita",
               not r.get("ok") and "somente leitura" in r.get("erro", ""),
               r.get("erro", json.dumps(r))[:150])
        afirma("e o phx-b continua com as 25 linhas",
               cb.call({"op": "varrer", "database": "rh",
                        "tabela": "funcionarios", "max": 1})["registros"] == 25, 25)

        print("\n-- 8. a credencial sobrevive a EDICAO da ligacao")
        # A tela nunca recebe token nem senha de volta ("(oculto)"), entao um
        # salvar comum vem sem os dois. Sem `com_o_token_de`, editar a
        # descricao apagaria a chave da porta da rede -- e a ligacao pararia
        # de conectar com "token invalido", que manda procurar a senha.
        ca.call({"op": "dblink_salvar", "nome": "b", "motor": "phxsql",
                 "host": "127.0.0.1", "porta": PORTA_B, "usuario": "root",
                 "database": "rh", "descricao": "o RH da filial",
                 "somente_leitura": True})
        r = ca.bruto({"op": "dblink_testar", "dblink": "b"})
        afirma("editar a ligacao sem mandar token nem senha nao quebra a ligacao",
               r.get("ok"), r.get("erro", "conectou") if not r.get("ok") else "conectou")

        print("\n-- 8b. sem token nenhum, o erro DIZ onde o token se grava")
        # A tela ainda nao tem campo de token: sem esta frase, quem criar a
        # ligacao por la recebe «token invalido» e vai procurar a SENHA.
        ca.call({"op": "dblink_salvar", "nome": "sem-token", "motor": "phxsql",
                 "host": "127.0.0.1", "porta": PORTA_B, "usuario": "root",
                 "senha": SENHA_B, "database": "rh"})
        r = ca.bruto({"op": "dblink_testar", "dblink": "sem-token"})
        afirma("ligacao sem token diz qual campo falta, e nao so `token invalido`",
               not r.get("ok") and "token_remoto" in r.get("erro", ""),
               r.get("erro", json.dumps(r))[:200])

        print("\n-- 9. a credencial errada e o que ela NAO derruba")
        ca.call({"op": "dblink_salvar", "nome": "ruim", "motor": "phxsql",
                 "host": "127.0.0.1", "porta": PORTA_B, "token_remoto": "token-errado",
                 "usuario": "root", "senha": SENHA_B, "database": "rh"})
        r = ca.bruto({"op": "dblink_testar", "dblink": "ruim"})
        afirma("token ERRADO continua dizendo `token invalido` -- a frase de "
               "ajuda so troca quando o token esta VAZIO",
               not r.get("ok") and "token invalido" in r.get("erro", ""),
               r.get("erro", json.dumps(r))[:150])
        ca.call({"op": "dblink_salvar", "nome": "ruim", "motor": "phxsql",
                 "host": "127.0.0.1", "porta": PORTA_B, "token_remoto": TOKEN_B,
                 "usuario": "root", "senha": "senha-errada", "database": "rh"})
        r = ca.bruto({"op": "dblink_testar", "dblink": "ruim"})
        afirma("senha errada recusa depois do token, e o erro diz login",
               not r.get("ok") and "login" in r.get("erro", "").lower(),
               r.get("erro", json.dumps(r))[:150])

        # A porta que nao fala o nosso protocolo: o `dblink_testar` tem de
        # devolver erro, e o phx-a tem de continuar atendendo outra conexao.
        ca.call({"op": "dblink_salvar", "nome": "morto", "motor": "phxsql",
                 "host": "127.0.0.1", "porta": PORTA_A + 900, "token_remoto": "x",
                 "usuario": "root", "senha": "y", "database": "rh",
                 "timeout_s": 3})
        comeco = time.time()
        r = ca.bruto({"op": "dblink_testar", "dblink": "morto"})
        gasto = time.time() - comeco
        outra = Cliente(a)
        afirma("porta morta devolve erro em vez de pendurar o servidor",
               not r.get("ok") and gasto < 15, f"{gasto:.2f} s")
        afirma("e outra conexao ao phx-a continua entrando",
               outra.call({"op": "bancos"}) == ["loja"], "sim")

        print("\n-- 10. a queda da conexao do lado do phx-b")
        # `socket.makefile()` segura o descritor: fechar so o soquete deixa o
        # fd aberto e o outro lado nunca ve o fim. E a licao do BULKINSERT.
        antes = ca.call({"op": "dblink_testar", "dblink": "b"})["ms"]
        b.parar()
        r = ca.bruto({"op": "dblink_testar", "dblink": "b"})
        afirma("com o phx-b fora do ar o dblink diz que nao conectou",
               not r.get("ok"), r.get("erro", json.dumps(r))[:130])
        afirma("e o phx-a continua vivo e respondendo",
               ca.call({"op": "bancos"}) == ["loja"], f"testar levava {antes} ms")
    finally:
        b.parar()
        a.parar()
        shutil.rmtree(TRABALHO, ignore_errors=True)

    print()
    if falhas:
        print(f"REPROVOU em {len(falhas)}: " + ", ".join(falhas))
        sys.exit(1)
    print("as conferencias passaram todas.")


if __name__ == "__main__":
    principal()
