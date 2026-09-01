#!/usr/bin/env python3
"""A prova do DbLink contra um MySQL(R) DE VERDADE.

    python3 bancada/dblink/prova-mysql.py

Gemea da `prova-postgres.py`, e pelo mesmo motivo: o cliente do MySQL(R)
estava provado contra a `prova-sincronia.py` -- que exercita a CONVERGENCIA de
duas tabelas -- e contra testes de unidade, mas nao havia nada que conferisse
cada resposta das operacoes de CATALOGO contra o que o proprio MySQL(R)
responde.

Como ela prova
--------------
Cada resposta do PhxSql e conferida contra o `mysql`, que e o ORACULO
INDEPENDENTE: dois codigos sem uma linha em comum tem de dizer a mesma coisa.
Conferir contra o que este script espera provaria so que o script e o servidor
concordam.

O oraculo fala pelo MESMO caminho que o nosso cliente: TCP em 127.0.0.1:3306,
com o usuario e a senha da prova, e com `--default-character-set=utf8mb4`.
Esse ultimo pedaco custou uma hipotese inteira -- ver «A armadilha do
oraculo», no `LEIA-ME.md`.

O que ela sobe e o que ela nao mata
-----------------------------------
Um `phxsqld` proprio, em porta propria (7490), morto pelo PID -- nunca
`pkill`, que derrubaria o servidor de outra frente na mesma maquina. O
`mysqld` ela NAO sobe nem derruba: se nao estiver no ar, ela diz o comando e
para. A base da prova ela monta se faltar, pelo soquete local do root.

Degradar limpo
--------------
Nenhuma falha estoura com traceback: ela e registrada, o que dependia dela e
PULADO dizendo que foi pulado, e a prova chega ao fim com o numero. Prova que
estoura diz menos do que prova que reprova.
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
TRABALHO = Path(os.environ.get("PHX_TRABALHO", f"/tmp/phx-my-{os.getpid()}"))

PORTA = int(os.environ.get("PORTA", "7490"))
TOKEN = "prova-mysql"
SENHA = "prova-9876"

MY_HOST, MY_PORTA = "127.0.0.1", 3306
MY_USUARIO, MY_SENHA, MY_BASE = "phxprova", "prova-1234", "bancada_phx"

# Os esquemas que o dialeto tira quando a ligacao nao tem base padrao.
SISTEMA = ("mysql", "information_schema", "performance_schema", "sys")

falhas = []
pulados = []


# ------------------------------------------------------ o oraculo: o mysql


def mysql(sql, base=MY_BASE, cabecalho=False):
    """Uma pergunta ao MySQL pelo cliente OFICIAL dele, por TCP e com senha --
    o mesmo caminho que o nosso cliente usa, para a comparacao ser entre
    motores e nao entre transportes.

    `--default-character-set=utf8mb4` nao e detalhe: o `mysql` desta maquina
    abre em `latin1`, e sem isto «Itajai» com acento volta diferente do
    oraculo por culpa do ORACULO. Ver o LEIA-ME.
    """
    cmd = ["mysql", "-h", MY_HOST, "-P", str(MY_PORTA), "-u", MY_USUARIO,
           "--default-character-set=utf8mb4", "-B"]
    if not cabecalho:
        cmd.append("-N")
    if base:
        cmd.append(base)
    # A senha vai por ambiente, e nao em `-p...`: em `-p` ela aparece no `ps`
    # de quem estiver na maquina, e o proprio cliente avisa disso -- o aviso
    # entao entulha o stderr e ESCONDE o erro de verdade quando ha um. E o
    # mesmo caminho que a prova do PostgreSQL(R) usa com `PGPASSWORD`.
    return rodar(cmd + ["-e", sql], MY_SENHA).strip("\n")


class Oraculo(Exception):
    """O oraculo nao respondeu. Nunca vira traceback: vira falha nomeada."""


def rodar(cmd, senha=None):
    """Chama o cliente oficial. Toda saida ruim vira `Oraculo`, inclusive o
    binario que nao existe -- `FileNotFoundError` cru viraria traceback, e
    prova que estoura diz menos do que prova que reprova."""
    ambiente = dict(os.environ)
    if senha is not None:
        ambiente["MYSQL_PWD"] = senha
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, env=ambiente)
    except OSError as e:
        raise Oraculo(f"nao consegui chamar o cliente `mysql`: {e}") from e
    if r.returncode != 0:
        raise Oraculo(r.stderr.strip()[:400] or f"saida {r.returncode}")
    return r.stdout


def raiz(sql, base=""):
    """O root pelo SOQUETE local, so para MONTAR a base da prova.

    Nenhuma conferencia passa por aqui: o oraculo e o usuario da prova, pelo
    mesmo transporte do nosso cliente.
    """
    cmd = ["mysql", "--default-character-set=utf8mb4"]
    if base:
        cmd.append(base)
    return rodar(cmd + ["-e", sql]).strip()


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


def confere(rotulo, phx, my):
    """O PhxSql contra o mysql. Nunca contra o que este script acha."""
    ok = phx == my
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}")
    print(f"        phxsql: {phx}")
    print(f"        mysql:  {my}")
    if not ok:
        falhas.append(rotulo)


def afirma(rotulo, condicao, visto):
    ok = bool(condicao)
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto}")
    if not ok:
        falhas.append(rotulo)


def pular(rotulo, porque):
    """O que dependia de algo que caiu. Ele APARECE, e nao some da conta."""
    print(f"  --   PULADO {rotulo}: {porque}")
    pulados.append(rotulo)


def grade(linhas):
    """As linhas do PhxSql escritas como o `mysql -N -B` as escreve.

    O `-B` separa celula por tabulacao e escreve o NULO como a palavra
    `NULL` -- que e justamente como ele o separa da cadeia vazia. Sem essa
    traducao a comparacao acusaria diferenca onde ha acordo.
    """
    return "\n".join(
        "\t".join("NULL" if v is None else str(v) for v in l) for l in linhas
    )


def observa(rotulo, visto):
    """Um numero que nao e veredito, e sim informacao medida."""
    print(f"  ··   {rotulo}: {visto}")


def secao(titulo, corpo):
    """Cada secao degrada sozinha: o oraculo que nao respondeu vira falha
    nomeada e a prova segue para a proxima, em vez de morrer no meio."""
    print(f"\n-- {titulo}")
    try:
        corpo()
    except Oraculo as e:
        afirma(f"{titulo}: o oraculo respondeu", False, f"mysql falhou: {e}")
    except Exception as e:  # noqa: BLE001 -- degradar limpo e o requisito
        afirma(f"{titulo}: a secao rodou inteira", False,
               f"{type(e).__name__}: {e}")


# ------------------------------------------------------ a base da prova


ESQUEMA = """
CREATE TABLE clientes (
  id       INT NOT NULL,
  nome     VARCHAR(40) NOT NULL,
  cidade   VARCHAR(20) COMMENT 'Cidade do cliente',
  saldo    DECIMAL(15,2),
  ativo    TINYINT(1),
  cadastro DATE,
  apelido  VARCHAR(20) DEFAULT NULL,
  PRIMARY KEY (id),
  KEY por_cidade (cidade)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='Cadastro de clientes da bancada';
INSERT INTO clientes VALUES
  (1,'Ana Prado','Blumenau',1500.50,1,'2024-10-04','ana'),
  (2,'Bruno Reis','Joinville',2750.00,0,'2024-11-15',NULL),
  (3,'Carla Lima','Itajai',980.25,1,'2025-01-20',''),
  (4,'Diego Souza','Curitiba',12000.75,1,'2025-03-08',NULL),
  (5,'Elisa Nunes','Chapeco',430.00,0,'2025-06-30','lisa');
UPDATE clientes SET cidade='Itajaí'  WHERE id=3;
UPDATE clientes SET cidade='Chapecó' WHERE id=5;
CREATE TABLE sem_analise (id INT PRIMARY KEY, texto VARCHAR(10))
  ENGINE=InnoDB STATS_AUTO_RECALC=0;
INSERT INTO sem_analise VALUES (1,'um'),(2,'dois'),(3,'tres');
SET SESSION cte_max_recursion_depth = 5000;
INSERT INTO sem_analise
  SELECT n+3, CONCAT('t',n) FROM (
    WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM s WHERE n<2000)
    SELECT n FROM s) x;
"""


def montar_a_base():
    """Monta a base da prova se ela faltar. O SQL esta no LEIA-ME.

    A ordem importa: o `sem_analise` recebe 2.000 linhas DEPOIS de nascer com
    tres e com `STATS_AUTO_RECALC=0`, e e isso que deixa o `TABLE_ROWS`
    parado em 3 -- a tabela nunca analisada, que e o que faz a conferencia da
    ESTIMATIVA valer alguma coisa.
    """
    raiz(
        f"CREATE DATABASE IF NOT EXISTS {MY_BASE} "
        "CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci;"
        f"CREATE USER IF NOT EXISTS '{MY_USUARIO}'@'%' "
        f"IDENTIFIED WITH mysql_native_password BY '{MY_SENHA}';"
        f"ALTER USER '{MY_USUARIO}'@'%' "
        f"IDENTIFIED WITH mysql_native_password BY '{MY_SENHA}';"
        f"GRANT ALL ON {MY_BASE}.* TO '{MY_USUARIO}'@'%';FLUSH PRIVILEGES;"
    )
    tem = raiz("SHOW TABLES LIKE 'clientes'", MY_BASE)
    if not tem:
        raiz(ESQUEMA, MY_BASE)


# ------------------------------------------------------------- as secoes


def principal():
    if not PHXSQLD.exists():
        raise SystemExit(
            f"nao achei {PHXSQLD}.\nRode `cargo build --release` antes."
        )
    try:
        socket.create_connection((MY_HOST, MY_PORTA), timeout=3).close()
    except OSError:
        raise SystemExit(
            "o MySQL nao esta no ar em 127.0.0.1:3306.\n"
            "  service mysql start\n"
            "Esta prova NAO sobe nem derruba o mysqld: derrubar o banco de\n"
            "outra frente na mesma maquina custa mais do que a prova vale."
        )
    try:
        montar_a_base()
    except Oraculo as e:
        raise SystemExit(
            f"nao consegui montar a base da prova: {e}\n"
            "O SQL esta no cabecalho de `bancada/dblink/LEIA-ME.md`; rode-o\n"
            "como root e chame esta prova de novo."
        )

    versao_my = mysql("SELECT VERSION()")
    print(f"=== DbLink contra MySQL(R) {versao_my} de verdade ===")

    servidor = Phxsqld()
    try:
        return correr(servidor)
    finally:
        servidor.parar()
        shutil.rmtree(TRABALHO, ignore_errors=True)


def correr(_servidor):
    c = Cliente()
    c.call({"op": "login", "usuario": "root", "senha": SENHA})
    c.call({"op": "dblink_salvar", "nome": "my", "motor": "mysql",
            "host": MY_HOST, "porta": MY_PORTA, "usuario": MY_USUARIO,
            "senha": MY_SENHA, "database": MY_BASE, "somente_leitura": True})
    # A MESMA ligacao, sem base padrao. E o caso que a prova achou quebrado:
    # `dblink_salvar` aceita `database` vazio, e um servidor de MySQL(R)
    # enxerga todas as bases -- entao a ligacao sem base padrao e a forma
    # natural de navegar por varias.
    c.call({"op": "dblink_salvar", "nome": "semdb", "motor": "mysql",
            "host": MY_HOST, "porta": MY_PORTA, "usuario": MY_USUARIO,
            "senha": MY_SENHA, "database": "", "somente_leitura": True})

    estado = {}

    # ---------------------------------------------------------------- 0
    def oraculo_honesto():
        """O oraculo tem de falar utf8mb4, ou a comparacao e entre
        TRANSPORTES e nao entre motores. Custou uma hipotese inteira."""
        confere("o oraculo abre em utf8mb4 (senao o acento e culpa DELE)",
                "utf8mb4", mysql("SELECT @@character_set_client"))
        confere("e a base da prova tambem",
                "utf8mb4", mysql(
                    "SELECT DEFAULT_CHARACTER_SET_NAME FROM information_schema.SCHEMATA"
                    f" WHERE SCHEMA_NAME='{MY_BASE}'"))

    secao("0. o oraculo antes das perguntas", oraculo_honesto)

    # ---------------------------------------------------------------- 1
    def testar():
        res = c.call({"op": "dblink_testar", "dblink": "my"}).get("resultado", {})
        confere("com quem o outro lado acha que fala",
                res.get("usuario_efetivo"), mysql("SELECT CURRENT_USER()"))
        confere("em que base caiu", res.get("database"), mysql("SELECT DATABASE()"))
        # A versao que o DbLink publica vem do APERTO DE MAO, e nao de um
        # `SELECT`: se as duas nao baterem, uma das duas foi inventada.
        confere("a versao do aperto de mao e a que o servidor diz ter",
                res.get("versao"), versao_do_servidor := mysql("SELECT VERSION()"))
        afirma("e ela e mesmo de um 8.x",
               str(versao_do_servidor).startswith("8."), versao_do_servidor)
        afirma("o id da conexao do outro lado veio",
               isinstance(res.get("conexao_id"), int) and res["conexao_id"] > 0,
               res.get("conexao_id"))

    secao("1. dblink_testar: com quem, em que base, e qual versao", testar)

    # ---------------------------------------------------------------- 2
    def bancos():
        b = c.call({"op": "dblink_bancos", "dblink": "my"}).get("resultado", {})
        nomes = sorted(x if isinstance(x, str) else x.get("nome")
                       for x in b.get("bancos", []))
        # `SHOW DATABASES` mostra o que o USUARIO enxerga -- e o oraculo e o
        # mesmo usuario, entao as duas listas tem de ser identicas. Perguntar
        # como root aqui compararia dois direitos diferentes.
        confere("bancos", ",".join(nomes),
                ",".join(sorted(mysql("SHOW DATABASES", base="").splitlines())))

    secao("2. dblink_bancos: o que ESTE usuario enxerga", bancos)

    # ---------------------------------------------------------------- 3
    def tabelas():
        esperado = ",".join(sorted(mysql(
            "SELECT TABLE_NAME FROM information_schema.TABLES"
            f" WHERE TABLE_SCHEMA='{MY_BASE}'").splitlines()))
        tabs = []
        for rotulo, pedido in (
            ("sem database (o padrao)", {"op": "dblink_tabelas", "dblink": "my"}),
            ("com o database, como a TELA manda",
             {"op": "dblink_tabelas", "dblink": "my", "database": MY_BASE}),
        ):
            tabs = c.call(pedido).get("resultado", {}).get("tabelas", [])
            confere(f"tabelas — {rotulo}",
                    ",".join(sorted(t["nome"] for t in tabs)), esperado)
        estado["tabelas"] = tabs

        # O `next` vira busca com padrao: com o defeito reposto a lista vem
        # VAZIA, e uma prova que estoura aqui diz menos que uma que reprova.
        t = next((x for x in tabs if x["nome"] == "clientes"), None)
        if t is None:
            afirma("a tabela `clientes` esta na lista", False, "nao veio")
            pular("comentario, esquema, motor, estimativa e bytes",
                  "dependem da linha de `clientes`")
            return
        afirma("a tabela `clientes` esta na lista", True, t["nome"])
        confere("o comentario da tabela", t.get("comentario"), mysql(
            "SELECT TABLE_COMMENT FROM information_schema.TABLES"
            f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"))
        confere("o esquema em que ela mora", t.get("schema"), mysql(
            "SELECT TABLE_SCHEMA FROM information_schema.TABLES"
            f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"))
        confere("o motor de armazenamento", t.get("motor"), mysql(
            "SELECT ENGINE FROM information_schema.TABLES"
            f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"))
        confere("o tipo (tabela ou vista)", t.get("tipo"), mysql(
            "SELECT TABLE_TYPE FROM information_schema.TABLES"
            f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"))
        confere("os bytes (dados + indices)", str(t.get("bytes")), mysql(
            "SELECT DATA_LENGTH + INDEX_LENGTH FROM information_schema.TABLES"
            f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"))

    secao("3. dblink_tabelas, pelos dois caminhos", tabelas)

    # ---------------------------------------------------------------- 4
    def estimativa():
        """`registros_estimados` tem de ser o que o SERVIDOR estimou, e nao
        uma contagem -- e a `sem_analise` existe para que os dois numeros
        sejam diferentes de proposito. Se ela nao os separasse, a conferencia
        passaria por vacuidade."""
        tabs = estado.get("tabelas") or []
        t = next((x for x in tabs if x["nome"] == "sem_analise"), None)
        if t is None:
            afirma("a tabela nunca analisada esta na lista", False, "nao veio")
            pular("a conferencia da ESTIMATIVA", "sem a linha de `sem_analise`")
            return
        estimado = mysql("SELECT TABLE_ROWS FROM information_schema.TABLES"
                         f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='sem_analise'")
        contado = mysql("SELECT COUNT(*) FROM sem_analise")
        afirma("o servidor real separa estimativa de contagem",
               estimado != contado, f"TABLE_ROWS={estimado} e COUNT(*)={contado}")
        confere("e o DbLink publica a ESTIMATIVA do servidor, nao a contagem",
                str(t.get("registros_estimados")), estimado)
        observa("o desvio da estimativa nesta tabela",
                f"{int(contado) / max(int(estimado), 1):.0f}x")

    secao("4. a tabela nunca analisada: estimativa nao e contagem", estimativa)

    # ---------------------------------------------------------------- 5
    def estrutura():
        est = c.call({"op": "dblink_estrutura", "dblink": "my",
                      "tabela": "clientes"}, exigir=False)
        if not est.get("ok"):
            afirma("dblink_estrutura foi aceito", False, est.get("erro"))
            pular("colunas, tipos, chave, comentario e indices",
                  "a estrutura nao veio")
            return
        est = est.get("resultado", {})
        col = est.get("colunas") or {}
        idx = est.get("indices") or {}

        # A FORMA da resposta, conferida contra o cabecalho que o proprio
        # `mysql` imprime. E o que obriga o cliente a ler por NOME: do lado
        # do MySQL(R) sao as nove colunas do `SHOW FULL COLUMNS`, e nao as
        # seis do outro motor.
        confere("a forma da resposta de colunas e a do SHOW FULL COLUMNS",
                ",".join(x["nome"] for x in col.get("colunas", [])),
                ",".join(mysql("SHOW FULL COLUMNS FROM `clientes`",
                               cabecalho=True).splitlines()[0].split("\t")))
        confere("a forma da resposta de indices e a do SHOW INDEX",
                ",".join(x["nome"] for x in idx.get("colunas", [])),
                ",".join(mysql("SHOW INDEX FROM `clientes`",
                               cabecalho=True).splitlines()[0].split("\t")))

        # Daqui para baixo a leitura e POR NOME, que e como um cliente certo
        # le: comparar por posicao quebraria calado no dia em que o MySQL(R)
        # acrescentar uma coluna ao `SHOW`.
        def por_nome(bloco):
            onde = {x["nome"]: i for i, x in enumerate(bloco.get("colunas", []))}
            return lambda l, nome: (l[onde[nome]] if nome in onde else None)

        cel, celi = por_nome(col), por_nome(idx)
        linhas = col.get("linhas") or []
        if not linhas:
            afirma("as colunas de `clientes` vieram", False, "lista vazia")
            pular("nomes, tipos, chave e comentario da coluna", "sem colunas")
        else:
            confere("colunas, na ordem",
                    ",".join(cel(l, "Field") for l in linhas),
                    ",".join(mysql(
                        "SELECT COLUMN_NAME FROM information_schema.COLUMNS"
                        f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"
                        " ORDER BY ORDINAL_POSITION").splitlines()))
            confere("tipos, como o proprio MySQL os escreve",
                    ",".join(cel(l, "Type") for l in linhas),
                    ",".join(mysql(
                        "SELECT COLUMN_TYPE FROM information_schema.COLUMNS"
                        f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"
                        " ORDER BY ORDINAL_POSITION").splitlines()))
            confere("quem aceita nulo",
                    ",".join(cel(l, "Null") for l in linhas),
                    ",".join(mysql(
                        "SELECT IS_NULLABLE FROM information_schema.COLUMNS"
                        f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"
                        " ORDER BY ORDINAL_POSITION").splitlines()))
            confere("chave primaria",
                    ",".join(cel(l, "Field") for l in linhas
                             if cel(l, "Key") == "PRI"),
                    mysql("SELECT COLUMN_NAME FROM information_schema.COLUMNS"
                          f" WHERE TABLE_SCHEMA='{MY_BASE}' AND TABLE_NAME='clientes'"
                          " AND COLUMN_KEY='PRI' ORDER BY ORDINAL_POSITION"))
            confere("o comentario da COLUNA",
                    next((cel(l, "Comment") for l in linhas
                          if cel(l, "Field") == "cidade"), "(sem a coluna)"),
                    mysql("SELECT COLUMN_COMMENT FROM information_schema.COLUMNS"
                          f" WHERE TABLE_SCHEMA='{MY_BASE}'"
                          " AND TABLE_NAME='clientes' AND COLUMN_NAME='cidade'"))

        li = idx.get("linhas") or []
        if not li:
            afirma("os indices de `clientes` vieram", False, "lista vazia")
            pular("nome, coluna e unicidade dos indices", "sem indices")
        else:
            # Contra o CATALOGO, que e um caminho diferente do `SHOW`: aqui a
            # conferencia e de CONTEUDO, e os dois lados sao ordenados porque
            # `SHOW INDEX` devolve na ordem de criacao (PRIMARY primeiro) e o
            # `information_schema` na que se pedir. Ordem se confere logo
            # abaixo, contra o proprio `SHOW`.
            confere("indices: nome, coluna e unicidade (contra o catalogo)",
                    ";".join(sorted(
                        f"{celi(l, 'Key_name')}.{celi(l, 'Column_name')}"
                        f"={celi(l, 'Non_unique')}" for l in li)),
                    ";".join(sorted(
                        f"{a}.{b}={n}" for a, b, n in (
                            x.split("\t") for x in mysql(
                                "SELECT INDEX_NAME, COLUMN_NAME, NON_UNIQUE"
                                " FROM information_schema.STATISTICS"
                                f" WHERE TABLE_SCHEMA='{MY_BASE}'"
                                " AND TABLE_NAME='clientes'").splitlines()))))

        # E a prova mais dura das duas: a GRADE INTEIRA, celula por celula,
        # contra o que o cliente oficial imprime para a MESMA instrucao.
        # Ela confere de uma vez a ordem das linhas, a ordem das colunas, o
        # texto de cada celula e a diferenca entre NULO e cadeia vazia -- que
        # e onde um erro de leitura do protocolo apareceria.
        for rotulo, bloco, instrucao in (
            ("colunas", col, "SHOW FULL COLUMNS FROM `clientes`"),
            ("indices", idx, "SHOW INDEX FROM `clientes`"),
        ):
            confere(f"a grade de {rotulo}, celula por celula, contra o cliente oficial",
                    grade(bloco.get("linhas") or []), mysql(instrucao))

    secao("5. dblink_estrutura: forma, colunas, tipos, chave e indices",
          estrutura)

    # ---------------------------------------------------------------- 6
    def ler():
        bruto = c.call({"op": "dblink_ler", "dblink": "my", "tabela": "clientes",
                        "ordem": "id", "limite": 100}, exigir=False)
        if not bruto.get("ok"):
            afirma("dblink_ler foi aceito pelo servidor", False, bruto.get("erro"))
            pular("contagem, soma, acento, NULO, booleano e decimais",
                  "a leitura nao veio")
            return
        r = bruto.get("resultado", {})
        linhas = r.get("linhas", [])
        nomes = [x["nome"] for x in r.get("colunas", [])]
        estado["colunas_ler"] = r.get("colunas", [])
        col = {n: i for i, n in enumerate(nomes)}
        v = lambda l, n: l[col[n]]  # noqa: E731 -- ler por NOME, nunca por posicao

        confere("linhas lidas", str(len(linhas)),
                mysql("SELECT COUNT(*) FROM clientes"))
        # A soma sai do TEXTO que veio, com as casas que vieram: somar em
        # float esconderia justamente o arredondamento que o campo
        # `decimais` existe para impedir.
        confere("soma de saldo, com as casas que vieram",
                f"{sum(float(v(l, 'saldo')) for l in linhas):.2f}" if linhas else "(sem linhas)",
                f"{float(mysql('SELECT SUM(saldo) FROM clientes')):.2f}")
        confere("o saldo veio com as duas casas, e nao arredondado",
                ",".join(v(l, "saldo") for l in linhas),
                ",".join(mysql("SELECT saldo FROM clientes ORDER BY id").splitlines()))
        # O acento e a prova do utf8mb4 no fio: o nosso cliente pede o
        # conjunto 45 no aperto de mao, e o oraculo pede utf8mb4 na linha de
        # comando. Se um dos dois errar, «Itajai» sai diferente.
        confere("cidades, na ordem lida, COM acento",
                ",".join(v(l, "cidade") for l in linhas),
                ",".join(mysql("SELECT cidade FROM clientes ORDER BY id").splitlines()))
        # NULO e cadeia vazia sao coisas diferentes, e o `-N` do oraculo
        # imprime o NULO como a palavra NULL -- que e como ele os separa.
        confere("NULO e cadeia vazia continuam coisas diferentes",
                ",".join("NULL" if v(l, "apelido") is None else f"[{v(l, 'apelido')}]"
                         for l in linhas),
                ",".join(x if x == "NULL" else f"[{x}]" for x in mysql(
                    "SELECT apelido FROM clientes ORDER BY id").splitlines()))
        vistos = sorted({v(l, "ativo") for l in linhas})
        afirma("o booleano do MySQL e 1/0, e nao t/f", vistos == ["0", "1"], vistos)
        confere("e os verdadeiros sao os mesmos",
                ",".join(v(l, "id") for l in linhas if v(l, "ativo") == "1"),
                ",".join(mysql("SELECT id FROM clientes WHERE ativo ORDER BY id").splitlines()))

    secao("6. dblink_ler: o dado, o acento, o NULO e o booleano", ler)

    # ---------------------------------------------------------------- 7
    def decimais():
        """O campo `decimais` existe porque sem ele a tela arredondava um
        DECIMAL(15,2) de 15000,50 para 15.001. Ele tem de bater com a escala
        que o catalogo declara."""
        cols = estado.get("colunas_ler")
        if not cols:
            afirma("os metadados da leitura vieram", False, "nao vieram")
            pular("a escala do DECIMAL", "sem os metadados de `dblink_ler`")
            return
        saldo = next((x for x in cols if x["nome"] == "saldo"), None)
        if saldo is None:
            afirma("a coluna `saldo` veio nos metadados", False, "nao veio")
            pular("a escala do DECIMAL", "sem a coluna `saldo`")
            return
        confere("as casas decimais que a grade vai usar",
                str(saldo.get("decimais")), mysql(
                    "SELECT NUMERIC_SCALE FROM information_schema.COLUMNS"
                    f" WHERE TABLE_SCHEMA='{MY_BASE}'"
                    " AND TABLE_NAME='clientes' AND COLUMN_NAME='saldo'"))
        afirma("e a coluna esta marcada como numerica (a tela alinha a direita)",
               saldo.get("numerico") is True, saldo.get("numerico"))

    secao("7. o DECIMAL: as casas que a grade vai usar", decimais)

    # ---------------------------------------------------------------- 8
    def paginacao():
        """`tem_mais` sai de pedir uma linha a mais do que o teto. O erro
        classico aqui e o `LIMIT m, n` do MySQL(R), com os dois numeros
        trocados -- e ele so aparece quando o salto nao e zero."""
        p1 = c.call({"op": "dblink_ler", "dblink": "my", "tabela": "clientes",
                     "ordem": "id", "limite": 2}).get("resultado", {})
        confere("pagina 1 (limite 2)",
                ",".join(l[0] for l in p1.get("linhas", [])),
                ",".join(mysql("SELECT id FROM clientes ORDER BY id"
                               " LIMIT 2 OFFSET 0").splitlines()))
        afirma("e ela diz que ha mais", p1.get("tem_mais") is True, p1.get("tem_mais"))
        p3 = c.call({"op": "dblink_ler", "dblink": "my", "tabela": "clientes",
                     "ordem": "id", "limite": 2, "salto": 4}).get("resultado", {})
        confere("pagina 3 (limite 2, salto 4)",
                ",".join(l[0] for l in p3.get("linhas", [])),
                ",".join(mysql("SELECT id FROM clientes ORDER BY id"
                               " LIMIT 2 OFFSET 4").splitlines()))
        afirma("e a ultima pagina diz que acabou",
               p3.get("tem_mais") is False, p3.get("tem_mais"))
        pd = c.call({"op": "dblink_ler", "dblink": "my", "tabela": "clientes",
                     "ordem": "id", "descendente": True,
                     "limite": 3}).get("resultado", {})
        confere("e a ordem descendente e a do motor",
                ",".join(l[0] for l in pd.get("linhas", [])),
                ",".join(mysql("SELECT id FROM clientes ORDER BY id DESC"
                               " LIMIT 3 OFFSET 0").splitlines()))

    secao("8. paginacao e ordem: onde o LIMIT m,n apareceria", paginacao)

    # ---------------------------------------------------------------- 9
    def sem_base_padrao():
        """O defeito que esta prova achou, e que o teste de unidade nao podia
        achar sozinho: a ligacao salva SEM base padrao.

        `TABLE_SCHEMA = DATABASE()` com `DATABASE()` NULO nao casa com nada --
        e em SQL `x = NULL` nunca e verdadeiro. A lista vinha VAZIA, sem erro
        nenhum, que e o mesmo sintoma mudo do defeito do PostgreSQL(R).
        """
        t = c.call({"op": "dblink_tabelas", "dblink": "semdb"},
                   exigir=False)
        if not t.get("ok"):
            afirma("dblink_tabelas sem base padrao foi aceito", False, t.get("erro"))
            pular("a lista de tabelas sem base padrao", "a operacao foi recusada")
            return
        tabs = t.get("resultado", {}).get("tabelas", [])
        fora = "','".join(SISTEMA)
        esperado = ",".join(sorted(mysql(
            "SELECT CONCAT(TABLE_SCHEMA,'.',TABLE_NAME)"
            f" FROM information_schema.TABLES WHERE TABLE_SCHEMA NOT IN ('{fora}')",
            base="").splitlines()))
        afirma("a lista NAO vem vazia e calada", bool(tabs), f"{len(tabs)} tabela(s)")
        confere("e ela e o que este usuario enxerga fora dos esquemas de sistema",
                ",".join(sorted(f"{x['schema']}.{x['nome']}" for x in tabs)),
                esperado)
        # E a resposta carrega o que a proxima pergunta precisa: o `schema`
        # de cada linha e o `database` com que pedir a estrutura.
        if not tabs:
            pular("a volta pelo `schema` da linha", "a lista veio vazia")
            return
        alvo = next((x for x in tabs if x["nome"] == "clientes"), tabs[0])
        e = c.call({"op": "dblink_estrutura", "dblink": "semdb",
                    "database": alvo["schema"], "tabela": alvo["nome"]},
                   exigir=False)
        afirma("e com o `schema` da linha a estrutura abre",
               e.get("ok") is True, e.get("erro") or "abriu")

    secao("9. a ligacao SEM base padrao: o defeito que esta prova achou",
          sem_base_padrao)

    # --------------------------------------------------------------- 10
    def mudo_ou_barulhento():
        """O que sobra do caso sem base padrao: `estrutura` e `ler` sem
        `database` continuam falhando, e isso esta CERTO -- o que nao pode e
        falhar CALADO. A prova exige que o erro exista e nomeie a causa."""
        for op, extra in (("dblink_estrutura", {"tabela": "clientes"}),
                          ("dblink_ler", {"tabela": "clientes", "limite": 3})):
            r = c.call({"op": op, "dblink": "semdb", **extra}, exigir=False)
            erro = str(r.get("erro", ""))
            afirma(f"{op} sem base padrao falha DIZENDO, e nao calado",
                   (not r.get("ok")) and "database" in erro.lower(),
                   erro or "passou sem erro")

    secao("10. sem base padrao, o resto falha DIZENDO", mudo_ou_barulhento)

    print()
    if pulados:
        print(f"== {len(pulados)} conferencia(s) PULADA(S): " + "; ".join(pulados))
    if falhas:
        print(f"== REPROVOU em {len(falhas)}: " + "; ".join(falhas))
        return 1
    print("== passou: o dialeto e o cliente valem contra um MySQL(R) real")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(principal())
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001 -- prova que estoura diz menos
        print(f"\n== REPROVOU antes de comecar: {type(e).__name__}: {e}")
        sys.exit(2)
