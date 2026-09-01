#!/usr/bin/env python3
"""A prova do DbLink contra um PostgreSQL(R) DE VERDADE.

    python3 bancada/dblink/prova-postgres.py

Fecha o que o `docs/DBLINK.md` listava como pendente: o cliente e o dialeto
estavam provados contra um servidor de PROTOCOLO no soquete -- que responde o
que mandarem ele responder --, e nao contra um servidor real, que valida
sintaxe, tem `pg_class` e sabe se `unnest(...) WITH ORDINALITY` existe.

A premissa que caducou
----------------------
O documento dizia «nao ha PostgreSQL(R) instalado na maquina onde este codigo
foi escrito». Isso deixou de ser verdade: ha o 16.13, com `scram-sha-256` ja
no `pg_hba.conf` para 127.0.0.1. *A lista do que falta tambem e palpite ate
alguem medir* -- inclusive quando o palpite e nosso.

Como ela prova
--------------
Cada resposta do PhxSql e conferida contra o `psql`, que e o ORACULO
INDEPENDENTE: dois codigos sem uma linha em comum tem de dizer a mesma coisa.
Conferir contra o que este script espera provaria so que o script e o servidor
concordam.

O que ela sobe e o que ela nao mata
-----------------------------------
Um `phxsqld` proprio, em porta propria, morto pelo PID -- nunca `pkill`, que
derrubaria o servidor de outra frente na mesma maquina. O PostgreSQL ela NAO
sobe nem derruba: se nao estiver no ar, ela diz o comando e para.
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
# O PID no nome, que e o que o zelador guarda.
TRABALHO = Path(os.environ.get("PHX_TRABALHO", f"/tmp/phx-pg-{os.getpid()}"))

PORTA = int(os.environ.get("PORTA", "7480"))
TOKEN = "prova-postgres"
SENHA = "prova-9876"

PG_HOST, PG_PORTA = "127.0.0.1", 5432
PG_USUARIO, PG_SENHA, PG_BASE = "phxprova", "prova-1234", "bancada_phx"

falhas = []


# ------------------------------------------------------- o oraculo: o psql


def psql(sql, base=PG_BASE):
    """Uma pergunta ao PostgreSQL pelo cliente OFICIAL dele, por TCP e com
    senha -- o mesmo caminho que o nosso cliente usa, para a comparacao ser
    entre motores e nao entre transportes."""
    r = subprocess.run(
        ["psql", "-h", PG_HOST, "-p", str(PG_PORTA), "-U", PG_USUARIO,
         "-d", base, "-tAc", sql],
        capture_output=True, text=True,
        env=dict(os.environ, PGPASSWORD=PG_SENHA),
    )
    if r.returncode != 0:
        raise SystemExit(f"psql falhou:\n{r.stderr.strip()[:400]}")
    return r.stdout.strip()


# ------------------------------------------------------------- o phxsqld


class Phxsqld:
    """Morre pelo PID, e so ele."""

    def __init__(self):
        self.base = TRABALHO
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        cfg = {
            "base": "dados",
            "bind": f"127.0.0.1:{PORTA}",
            "token": TOKEN,
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": self.hash_da_senha(SENHA)},
            "usuarios": [],
        }
        (self.base / "config.json").write_text(json.dumps(cfg, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
        )
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", PORTA), timeout=2).close()
                return
            except OSError:
                pass
        raise SystemExit("o phxsqld nao subiu; veja " + str(self.base / "servidor.log"))

    @staticmethod
    def hash_da_senha(senha):
        r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                           capture_output=True, check=True)
        return r.stdout.decode().split('"')[3]

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Cliente:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORTA), timeout=60)
        self.f = self.s.makefile("rwb")

    def call(self, d, exigir=True):
        d.setdefault("token", TOKEN)
        self.s.sendall((json.dumps(d) + "\n").encode())
        r = json.loads(self.f.readline())
        if exigir and not r.get("ok"):
            raise SystemExit(f"{d['op']} recusado: " + json.dumps(r)[:400])
        return r


# ------------------------------------------------------------- o veredito


def confere(rotulo, phx, pg):
    """O PhxSql contra o psql. Nunca contra o que este script acha."""
    ok = phx == pg
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}")
    print(f"        phxsql: {phx}")
    print(f"        psql:   {pg}")
    if not ok:
        falhas.append(rotulo)


def afirma(rotulo, condicao, visto):
    ok = bool(condicao)
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto}")
    if not ok:
        falhas.append(rotulo)


def principal():
    if not PHXSQLD.exists():
        raise SystemExit(
            f"nao achei {PHXSQLD}.\nRode `cargo build --release` antes."
        )
    try:
        socket.create_connection((PG_HOST, PG_PORTA), timeout=3).close()
    except OSError:
        raise SystemExit(
            "o PostgreSQL nao esta no ar em 127.0.0.1:5432.\n"
            "  service postgresql start\n"
            "E a base da prova se monta com o cabecalho de `bancada/dblink/LEIA-ME.md`."
        )

    versao_pg = psql("SHOW server_version;")
    print(f"=== DbLink contra PostgreSQL(R) {versao_pg} de verdade ===")

    servidor = Phxsqld()
    try:
        c = Cliente()
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        c.call({"op": "dblink_salvar", "nome": "pg", "motor": "postgres",
                "host": PG_HOST, "porta": PG_PORTA, "usuario": PG_USUARIO,
                "senha": PG_SENHA, "database": PG_BASE, "somente_leitura": True})

        # 1. dblink_testar -- o aperto de mao SCRAM-SHA-256 contra o real.
        print("\n-- 1. dblink_testar: o SCRAM contra um servidor que confere de verdade")
        res = c.call({"op": "dblink_testar", "dblink": "pg"}).get("resultado", {})
        confere("com quem o outro lado acha que fala",
                res.get("usuario_efetivo"), psql("SELECT current_user;"))
        confere("em que base caiu",
                res.get("database"), psql("SELECT current_database();"))
        afirma("a versao veio do servidor, e nao de uma constante nossa",
               str(res.get("versao", "")).startswith("16."), res.get("versao"))

        # 2. dblink_bancos
        print("\n-- 2. dblink_bancos")
        bancos = c.call({"op": "dblink_bancos", "dblink": "pg"}).get("resultado", {})
        nomes = sorted(b if isinstance(b, str) else b.get("nome")
                       for b in bancos.get("bancos", []))
        confere("bancos", ",".join(nomes), ",".join(sorted(psql(
            "SELECT datname FROM pg_database WHERE NOT datistemplate").splitlines())))

        # 3. dblink_tabelas -- pelos DOIS caminhos, porque o da tela e o que
        #    estava quebrado: ela devolve o nome do BANCO no campo que o
        #    dialeto le como ESQUEMA.
        print("\n-- 3. dblink_tabelas, pelos dois caminhos")
        esperado = ",".join(sorted(psql(
            "SELECT tablename FROM pg_tables WHERE schemaname='public'").splitlines()))
        for rotulo, pedido in (
            ("sem database (o padrao)", {"op": "dblink_tabelas", "dblink": "pg"}),
            ("com o database, como a TELA manda",
             {"op": "dblink_tabelas", "dblink": "pg", "database": PG_BASE}),
        ):
            tabs = c.call(pedido).get("resultado", {}).get("tabelas", [])
            confere(f"tabelas — {rotulo}",
                    ",".join(sorted(t["nome"] for t in tabs)), esperado)
        # Com o defeito reposto a lista vem VAZIA, e uma prova que estoura
        # aqui diz menos do que uma que da o veredito: o `next` vira busca
        # com padrao, e o que depende dela e pulado DIZENDO que foi pulado.
        t = next((x for x in tabs if x["nome"] == "clientes"), None)
        if t is None:
            afirma("a tabela `clientes` esta na lista", False, "nao veio")
            print("  --   pulados: comentario, esquema e o reltuples,"
                  " que dependem dela")
        else:
            confere("o comentario da tabela", t.get("comentario"),
                    psql("SELECT obj_description('clientes'::regclass,'pg_class')"))
            confere("o esquema em que ela mora", t.get("schema"),
                    psql("SELECT schemaname FROM pg_tables WHERE tablename='clientes'"))

        # 4. dblink_estrutura -- o `format_type`, o `col_description` e o
        #    `pg_index` conferidos por um servidor que valida sintaxe.
        print("\n-- 4. dblink_estrutura: colunas, tipos, chave e comentario")
        est = c.call({"op": "dblink_estrutura", "dblink": "pg",
                      "tabela": "clientes"}).get("resultado", {})
        linhas = (est.get("colunas") or {}).get("linhas") or []
        if not linhas:
            afirma("a estrutura de `clientes` veio", False, "vazia")
            print("  --   pulados: tipos, chave e comentario da coluna")
            linhas = [["", "", "", "", "", ""]]
        confere("colunas, na ordem",
                ",".join(l[0] for l in linhas),
                ",".join(psql("SELECT column_name FROM information_schema.columns"
                              " WHERE table_name='clientes'"
                              " ORDER BY ordinal_position").splitlines()))
        confere("tipos, como o proprio PostgreSQL os escreve",
                ",".join(l[1] for l in linhas),
                ",".join(psql("SELECT format_type(a.atttypid, a.atttypmod)"
                              " FROM pg_attribute a"
                              " WHERE a.attrelid='clientes'::regclass"
                              " AND a.attnum > 0 AND NOT a.attisdropped"
                              " ORDER BY a.attnum").splitlines()))
        confere("chave primaria",
                ",".join(l[0] for l in linhas if l[3] == "PRI"),
                psql("SELECT a.attname FROM pg_index i"
                     " JOIN pg_attribute a ON a.attrelid=i.indrelid"
                     " AND a.attnum = ANY(i.indkey)"
                     " WHERE i.indrelid='clientes'::regclass AND i.indisprimary"))
        confere("o comentario da COLUNA",
                next((l[5] for l in linhas if l[0] == "cidade"), "(sem a coluna)"),
                psql("SELECT col_description('clientes'::regclass, 3)"))

        # 5. dblink_ler -- o dado, conferido pela soma.
        print("\n-- 5. dblink_ler: o dado, conferido pela SOMA")
        bruto = c.call({"op": "dblink_ler", "dblink": "pg", "tabela": "clientes",
                        "ordem": "id", "limite": 100}, exigir=False)
        if not bruto.get("ok"):
            afirma("dblink_ler foi aceito pelo servidor", False, bruto.get("erro"))
            linhas = []
        else:
            linhas = bruto.get("resultado", {}).get("linhas", [])
        confere("linhas lidas", str(len(linhas)), psql("SELECT count(*) FROM clientes"))
        confere("soma de saldo",
                f"{sum(float(l[3]) for l in linhas):.2f}" if linhas else "(sem linhas)",
                f"{float(psql('SELECT sum(saldo) FROM clientes')):.2f}")
        confere("cidades, na ordem lida",
                ",".join(l[2] for l in linhas),
                ",".join(psql("SELECT cidade FROM clientes ORDER BY id").splitlines()))

        # 6. O booleano. O `DBLINK.md` avisava que uma leitura ingenua
        #    (`== "1"`) trata TODO booleano do PostgreSQL(R) como falso, sem
        #    erro nenhum. Aqui da para VER o que ele manda.
        print("\n-- 6. o booleano do PostgreSQL nao e 1/0")
        vistos = sorted({l[4] for l in linhas})  # vazio se `ler` foi recusado
        afirma("ele manda t/f, e nao 1/0", vistos == ["f", "t"], vistos)
        confere("e os verdadeiros sao os mesmos",
                ",".join(l[0] for l in linhas if l[4] == "t"),
                ",".join(psql("SELECT id FROM clientes WHERE ativo ORDER BY id").splitlines()))

        # 7. O caso que o documento dizia faltar «ver acontecer»: da 14 em
        #    diante o `reltuples` de tabela nunca analisada e -1, e nao 0.
        print("\n-- 7. o reltuples da 14+: -1 para tabela nunca analisada")
        bruto = psql("SELECT reltuples FROM pg_class WHERE relname='clientes'")
        afirma("o servidor real devolve o -1 que so a 14+ devolve",
               float(bruto) == -1.0, bruto)
        if t is not None:
            afirma("e o DbLink NAO publica o -1 como contagem",
                   t.get("registros_estimados", -1) >= 0, t.get("registros_estimados"))

        print()
        if falhas:
            print(f"== REPROVOU em {len(falhas)}: " + "; ".join(falhas))
            return 1
        print("== passou: o dialeto e o cliente valem contra um PostgreSQL(R) real")
        return 0
    finally:
        servidor.parar()
        shutil.rmtree(TRABALHO, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(principal())
