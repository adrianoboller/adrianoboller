#!/usr/bin/env python3
"""O comparativo MEDIDO: o que o PhxSql ainda nao tem, contra quem tem.

    python3 bancada/comparativo/medir.py

# A regra que este medidor existe para honrar

*Veredito de ausencia se remede por data, nao por suspeita.* O `docs/HFSQL.md`
ja publicou CINCO vereditos de ausencia errados, e a §6 dele conta o padrao:
alguem escreveu «nao ha» num dia em que era verdade, e a frase ficou. Este
medidor achou o SEXTO -- a trava por linha, que existe desde que o
`travas.rs` entrou.

# Como cada celula e decidida

**Perguntando ao motor.** Quatro deles estao vivos nesta maquina, e a mesma
pergunta vai para os quatro na lingua de cada um: PhxSql (soquete, op `sql`),
MySQL(R), PostgreSQL(R) e SQLite(R). A instrucao passou -> `tem`. Recusou ->
`nao`, com a mensagem de recusa guardada. **Nao ha nenhuma celula destas
quatro colunas escrita a mao.**

A primeira versao deste medidor perguntava ao TEXTO do repositorio, e errou
tres vereditos na primeira corrida: `materializar a linha` num comentario virou
«view materializada», e o `ON DUPLICATE KEY UPDATE` que o DbLink manda para o
MySQL(R) virou upsert NOSSO. Sonda por padrao de texto acha o que nao e.

**Sonda de codigo**, so onde SQL nao alcanca (trava, TLS, PITR, direito por
coluna). Cada uma aponta arquivo e linha, e o texto do veredito diz o que a
sonda olhou.

**CITADO**, para quem nao esta aqui:

- **Cassandra(R)**: sem motor e sem fonte nesta maquina. O que se sabe vem do
  `docs/CASSANDRA.md`, lido no fonte da 5.0.10 (commit 7b5ab44) em sessao
  ANTERIOR.
- **HFSQL(R)**: sem motor, e a folha de 2013-10 do `docs/HFSQL.md` nao esta
  nesta sessao.

Citado nao e mentira; e afirmacao de segunda mao, e a tabela diz isso em cada
celula para que ninguem a leia como medida.
"""

import datetime
import json
import os
import pathlib
import re
import sqlite3
import subprocess
import sys
import tempfile

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ALVO = AQUI / "resultados.json"
PORTA = 7731

sys.path.insert(0, str(RAIZ / "bancada" / "utilizacao-padrao"))

TEM, MEIO, NAO, CITADO, SEM = "tem", "meio", "nao", "citado", "sem-motor"

# --------------------------------------------------- o defeito que se REPOE
#
# Portao entregue sem prova real e portao que ninguem sabe se pega. Aqui cada
# um tem o seu defeito de volta atras de uma variavel de ambiente, e o
# `prova-dos-portoes.py` roda o medidor uma vez por defeito exigindo que ele
# PARE. Prova nos dois sentidos: falha com o defeito reposto, passa sem ele.
DEFEITO = os.environ.get("PHX_CMP_DEFEITO", "")
DEFEITOS = {
    "indice-velho": "o indice volta ao formato `[{coluna: 0}]`, que o servidor "
                    "recusa -- e a tabela nao nasce",
    "envelope": "as sondas voltam a ler o envelope em vez do `resultado`",
    # NAO e' "pedir o catalogo sem `database`": eu achei que fosse, e a prova
    # real desmentiu -- sem `database` ele responde igual. O zero vinha do
    # ENVELOPE lido no nivel errado, e so. Entao o defeito reposto aqui e' o
    # que o portao realmente guarda: a lista chegar vazia.
    "catalogo-vazio": "a lista de operacoes chega vazia, venha de onde vier",
}
if DEFEITO and DEFEITO not in DEFEITOS:
    sys.exit(f"PHX_CMP_DEFEITO={DEFEITO!r} nao existe; ha {sorted(DEFEITOS)}")


# ------------------------------------------------------------- as perguntas
#
# Uma linha por capacidade. `sql` traz a instrucao na lingua de cada motor;
# `None` quer dizer «esta pergunta nao se faz por SQL a este motor», e ai vale
# a sonda de codigo. O `prep` roda antes e nao conta: e so a mesa posta.
PERGUNTAS = [
    ("view", "Visão (`CREATE VIEW`)",
     {"mysql": "CREATE VIEW v_c AS SELECT * FROM c",
      "postgres": "CREATE VIEW v_c AS SELECT * FROM c",
      "sqlite": "CREATE VIEW v_c AS SELECT * FROM c",
      "phxsql": None}),
    ("cte", "Subconsulta e CTE (`WITH … AS`)",
     {"mysql": "WITH x AS (SELECT * FROM c) SELECT * FROM x",
      "postgres": "WITH x AS (SELECT * FROM c) SELECT * FROM x",
      "sqlite": "WITH x AS (SELECT * FROM c) SELECT * FROM x",
      "phxsql": "WITH x AS (SELECT * FROM c) SELECT * FROM x"}),
    ("subconsulta", "Subconsulta no `WHERE`",
     {"mysql": "SELECT * FROM c WHERE id IN (SELECT id FROM c)",
      "postgres": "SELECT * FROM c WHERE id IN (SELECT id FROM c)",
      "sqlite": "SELECT * FROM c WHERE id IN (SELECT id FROM c)",
      "phxsql": "SELECT * FROM c WHERE id IN (SELECT id FROM c)"}),
    ("funcao_de_janela", "Função de janela (`OVER`)",
     {"mysql": "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM c",
      "postgres": "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM c",
      "sqlite": "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM c",
      "phxsql": "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM c"}),
    ("group_by", "`GROUP BY` com agregação",
     {"mysql": "SELECT cidade, COUNT(*) FROM c GROUP BY cidade",
      "postgres": "SELECT cidade, COUNT(*) FROM c GROUP BY cidade",
      "sqlite": "SELECT cidade, COUNT(*) FROM c GROUP BY cidade",
      "phxsql": "SELECT cidade, COUNT(*) FROM c GROUP BY cidade"}),
    ("expressao_no_where", "Expressão no `WHERE` (`preco * 1.1 > 100`)",
     {"mysql": "SELECT * FROM c WHERE preco * 1.1 > 100",
      "postgres": "SELECT * FROM c WHERE preco * 1.1 > 100",
      "sqlite": "SELECT * FROM c WHERE preco * 1.1 > 100",
      "phxsql": "SELECT * FROM c WHERE preco * 1.1 > 100"}),
    ("indice_parcial", "Índice parcial (`CREATE INDEX … WHERE`)",
     {"mysql": "CREATE INDEX ip ON c (id) WHERE id > 0",
      "postgres": "CREATE INDEX ip ON c (id) WHERE id > 0",
      "sqlite": "CREATE INDEX ip ON c (id) WHERE id > 0",
      "phxsql": None}),
    ("indice_por_expressao", "Índice por expressão (`lower(nome)`)",
     {"mysql": "CREATE INDEX ie ON c ((lower(nome)))",
      "postgres": "CREATE INDEX ie ON c (lower(nome))",
      "sqlite": "CREATE INDEX ie ON c (lower(nome))",
      "phxsql": None}),
    ("check_constraint", "Restrição `CHECK`",
     {"mysql": "CREATE TABLE ck (v INT CHECK (v > 0))",
      "postgres": "CREATE TABLE ck (v INT CHECK (v > 0))",
      "sqlite": "CREATE TABLE ck (v INT CHECK (v > 0))",
      "phxsql": None}),
    ("default_de_coluna", "`DEFAULT` de coluna",
     {"mysql": "CREATE TABLE df (v INT DEFAULT 7)",
      "postgres": "CREATE TABLE df (v INT DEFAULT 7)",
      "sqlite": "CREATE TABLE df (v INT DEFAULT 7)",
      "phxsql": None}),
    ("coluna_calculada", "Coluna calculada (`GENERATED ALWAYS AS`)",
     {"mysql": "CREATE TABLE gc (a INT, b INT GENERATED ALWAYS AS (a*2))",
      "postgres": "CREATE TABLE gc (a INT, b INT GENERATED ALWAYS AS (a*2) STORED)",
      "sqlite": "CREATE TABLE gc (a INT, b INT GENERATED ALWAYS AS (a*2))",
      "phxsql": None}),
    ("upsert", "Upsert (`ON CONFLICT` / `ON DUPLICATE KEY`)",
     {"mysql": "INSERT INTO c (id) VALUES (1) ON DUPLICATE KEY UPDATE id=id",
      "postgres": "INSERT INTO c (id) VALUES (1) ON CONFLICT (id) DO NOTHING",
      "sqlite": "INSERT INTO c (id) VALUES (1) ON CONFLICT (id) DO NOTHING",
      "phxsql": "INSERT INTO c (id) VALUES (1) ON CONFLICT (id) DO NOTHING"}),
    ("isolamento_acima_de_rc", "Nível de isolamento acima de `READ COMMITTED`",
     {"mysql": "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
      "postgres": "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
      "sqlite": None,  # SQLite nao tem o verbo: o isolamento e do journal
      "phxsql": "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"}),
]


# --------------------------------------------------- sondas que SQL nao faz
#
# Cada uma aponta arquivo e linha e diz o que olhou. A evidencia foi conferida
# a olho antes de virar sonda -- a primeira versao deste medidor provou que
# contagem de `grep` nao e veredito.
def sonda_codigo():
    def tem(rel, padrao):
        p = RAIZ / rel
        if not p.exists():
            return None
        for i, l in enumerate(p.read_text(encoding="utf-8", errors="replace").splitlines()):
            if re.search(padrao, l):
                return f"{rel}:{i + 1}"
        return None

    def citar(rel, padrao, quando_falta):
        """Caminho, linha E O TRECHO. Caminho pelado nao e evidencia.

        A sonda do TLS provou por que: ela apontava para `config.rs:709` e
        dava a impressao de falar do transporte de dados -- e aquele trecho e
        do cliente de E-MAIL. Com o trecho ao lado, o erro aparece na
        primeira leitura em vez de na terceira rodada.
        """
        p = RAIZ / rel
        if not p.exists():
            return quando_falta
        for i, l in enumerate(p.read_text(encoding="utf-8", errors="replace").splitlines()):
            if re.search(padrao, l):
                trecho = l.strip().lstrip("/ ").rstrip("\\").strip()
                return f"{rel}:{i + 1} — «{trecho}»"
        return quando_falta

    fora = {}

    # ACHADO DE 07/09/2026: existe, e o `docs/HFSQL.md` §3.3 dizia que nao.
    # Gestor proprio (intencao na tabela, exclusivo na linha), ligado ao
    # servidor como campo, com ordem canonica na abertura e LOCK TIMEOUT.
    ger = tem("crates/phxsql-server/src/travas.rs",
              r"intencao na tabela, exclusivo na linha")
    liga = tem("crates/phxsql-server/src/servidor.rs", r"travas: Mutex<")
    # Existir e ser TOMADA sao coisas diferentes, e uma sonda que so acha o
    # gestor provaria codigo morto. A terceira evidencia e o pedido da trava
    # DE LINHA no caminho de escrita.
    usa = tem("crates/phxsql-server/src/servidor.rs", r"Alvo::Linha\(rowid\)")
    fora["trava_por_linha"] = {
        "titulo": "Trava por linha nas transações",
        "phxsql": (TEM if (ger and liga and usa) else NAO,
                   f"gestor em {ger}; ligado em {liga}; pedida em {usa}"),
        # A trava global de dados continua existindo e serializando o motor
        # fora da transacao -- as duas coisas convivem, e dizer so uma mente.
        "nota": "vale DENTRO de transação: `esperar_trava` recusa com «sem "
                "transação» quem a pede fora dela, e aí a trava GLOBAL de "
                "dados serializa como antes",
    }

    # ERRO CORRIGIDO EM 07/09/2026: esta sonda apontava para `config.rs:709`,
    # que fala do cliente de E-MAIL e nao do transporte. Veredito certo,
    # evidencia errada -- e evidencia errada e a que sobrevive.
    ond = citar("crates/phxsql-server/src/rest.rs", r"TLS aqui",
                "sem menção de TLS no transporte")
    fora["tls_no_transporte"] = {
        "titulo": "TLS no transporte",
        "phxsql": (NAO, ond),
        "nota": "a cifra da porta de DADOS é própria (aperto estilo Noise) e "
                "existe; o que não existe é TLS, que exigiria crate — e zero "
                "dependências é pétrea",
    }

    ond = tem("crates/phxsql-server/src/usuarios.rs", r'"tabelas"')
    fora["direito_por_coluna"] = {
        "titulo": "Direito por COLUNA",
        "phxsql": (NAO, f"o portão lê `tabela`, e para aí: {ond}"),
        "nota": "o direito por tabela existe desde o pedido 124",
    }

    fora["pitr"] = {
        "titulo": "Recuperação a um ponto no tempo (PITR)",
        "phxsql": (NAO, "o backup é cópia inteira; o `.log` guarda o evento "
                        "mas não há quem o reaplique até um instante"),
        "nota": "o `.log` por tabela É o diário que um PITR usaria",
    }

    # A sonda tem DOIS lados desde 08/09/2026, e a citação de antes ficou
    # obsoleta: ela apontava para o comentário «nao ha parametros nem plano no
    # driver», que deixou de ser verdade quando o SQLBindParameter entrou.
    # Sonda que cita comentário morre quando o comentário é consertado — e
    # morre calada, imprimindo "sem menção" como se fosse a medida.
    #
    # O lado que FALTA é o do servidor, e ele se mede onde dói: o léxico
    # (`crates/phxsql-sql/src/lexico.rs`) ainda recusa o caractere `?`, então
    # nenhum `WHERE id = ?` chega a virar plano. Enquanto isso, é NÃO.
    #
    # E quando o `?` entrar no léxico esta sonda para em MEIO, nunca em TEM:
    # código dos dois lados não é efeito. Quem promove a célula é a sonda VIVA
    # (`bancada/odbc/prova-abi.py`, passo 7c), que confere a LINHA que voltou.
    lig = citar("crates/phxsql-odbc/src/lib.rs",
                r'extern "system" fn SQLBindParameter',
                "sem ligação de parâmetro no driver")
    lex = tem("crates/phxsql-sql/src/lexico.rs", r"'\?'")
    fora["parametro_no_prepared"] = {
        "titulo": "Parâmetro em instrução preparada (`?`)",
        "phxsql": (MEIO, f"o driver liga e manda `parametros`: {lig}; e o léxico "
                         f"já conhece o `?`: {lex} — falta a prova viva "
                         "(bancada/odbc/prova-abi.py, passo 7c)")
                  if lex else
                  (NAO, f"o driver já liga e manda `parametros` ({lig}), mas o "
                        "léxico do servidor recusa o caractere `?`: "
                        "crates/phxsql-sql/src/lexico.rs — «caractere nao faz "
                        "parte da linguagem»"),
        "nota": "o lado do driver está pronto e provado; falta a op sql ler "
                "`parametros` (frente F-CONSULTA)",
    }

    ck = tem("crates/phxsql-server/src/catalogo.rs", r'nome: "checksum"')
    fora["diff_de_dados"] = {
        "titulo": "Dizer ONDE duas tabelas diferem",
        "phxsql": (MEIO, f"o `checksum` diz SE diferem: {ck}"),
        "nota": "falta a operação que devolve as linhas divergentes",
    }

    return fora


# ------------------------------------------------------------- os motores
def por_sqlite(perguntas):
    saida = {}
    con = sqlite3.connect(":memory:")
    con.execute("CREATE TABLE c (id INTEGER PRIMARY KEY, nome TEXT, cidade TEXT, preco REAL)")
    for chave, _t, sql in perguntas:
        q = sql.get("sqlite")
        if q is None:
            saida[chave] = (SEM, "o verbo não existe neste motor")
            continue
        try:
            con.execute("SAVEPOINT s")
            con.execute(q)
            con.execute("ROLLBACK TO s")
            saida[chave] = (TEM, "aceitou")
        except Exception as e:
            saida[chave] = (NAO, str(e)[:90])
    return saida


def por_processo(perguntas, lingua, roda):
    saida = {}
    for chave, _t, sql in perguntas:
        q = sql.get(lingua)
        if q is None:
            saida[chave] = (SEM, "o verbo não existe neste motor")
            continue
        ok, msg = roda(q)
        saida[chave] = (TEM if ok else NAO, msg[:90])
    return saida


def mysql_roda(q):
    r = subprocess.run(["mysql", "-N", "-B", "cmp_phx", "-e", q],
                       capture_output=True, text=True)
    return r.returncode == 0, (r.stderr or "aceitou").strip()


def pg_roda(q):
    r = subprocess.run(["sudo", "-u", "postgres", "psql", "-d", "cmp_phx",
                        "-v", "ON_ERROR_STOP=1", "-c", q],
                       capture_output=True, text=True)
    return r.returncode == 0, (r.stderr or "aceitou").strip()


def prepara_mysql():
    subprocess.run(["mysql", "-e", "DROP DATABASE IF EXISTS cmp_phx; CREATE DATABASE cmp_phx"],
                   capture_output=True)
    return subprocess.run(
        ["mysql", "cmp_phx", "-e",
         "CREATE TABLE c (id INT PRIMARY KEY, nome VARCHAR(60), cidade VARCHAR(60), preco DECIMAL(12,2))"],
        capture_output=True).returncode == 0


def prepara_pg():
    subprocess.run(["sudo", "-u", "postgres", "psql", "-c",
                    "DROP DATABASE IF EXISTS cmp_phx"], capture_output=True)
    subprocess.run(["sudo", "-u", "postgres", "psql", "-c", "CREATE DATABASE cmp_phx"],
                   capture_output=True)
    return pg_roda("CREATE TABLE c (id INT PRIMARY KEY, nome TEXT, cidade TEXT, preco NUMERIC(12,2))")[0]


def por_phxsql(perguntas, base):
    """O NOSSO motor responde pelo soquete, como os outros pelo cliente deles.

    Sem atalho por dentro: a pergunta entra pela porta de dados, na op `sql`,
    que e o caminho que um cliente teria.
    """
    from oficina import Conexao, subir  # noqa: E402
    p = subir(base, PORTA)
    try:
        c = Conexao(PORTA)
        c.fala({"op": "criar_database", "database": "cmp"})
        mesa = c.fala({"op": "criar_tabela", "database": "cmp", "tabela": "c",
                "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                            {"nome": "nome", "tipo": "Str(60)"},
                            {"nome": "cidade", "tipo": "Str(60)"},
                            {"nome": "preco", "tipo": "Decimal(12,2)"}],
                "indices": [{"nome": "porId", "colunas": ["id"], "unico": True}]})
        if not mesa.get("ok"):
            raise SystemExit(f"MESA NAO POSTA: a tabela `c` nao nasceu -- "
                             f"{mesa.get('erro')}")
        saida = {}
        for chave, _t, sql in perguntas:
            q = sql.get("phxsql")
            if q is None:
                saida[chave] = (SEM, "o verbo não existe neste motor")
                continue
            r = c.fala({"op": "sql", "database": "cmp", "sql": q})
            ok = bool(r.get("ok"))
            saida[chave] = (TEM if ok else NAO,
                            (r.get("erro") or "aceitou")[:90])
        # ------------------------------------------------------------------
        # As seis que o tradutor SQL nao alcanca (ele traduz SELECT e as
        # rotinas do dialeto MySQL(R), nao DDL de tabela) vao ao PROTOCOLO --
        # e medindo o EFEITO, nunca o «aceitou».
        #
        # Campo desconhecido pode ser IGNORADO em silencio, e ai «aceitou»
        # viraria um `tem` falso. Entao cada sonda cria a tabela COM o campo,
        # exercita, e olha o que aconteceu com o DADO.
        # ------------------------------------------------------------------
        def cria(tab, colunas, indices=None, exigir=True):
            """Cria a tabela e, por padrao, EXIGE que ela tenha nascido.

            `exigir=False` e para as duas sondas em que a RECUSA e a
            propria resposta -- ali a tabela nao nascer e o veredito,
            e nao um medidor quebrado.

            Sem este `raise`, o medidor de 07/09/2026 rodou inteiro contra
            tabelas que nunca existiram: o indice ia como `[{"coluna": 0}]` e
            o servidor recusava com «indice pk sem colunas» -- e as sondas de
            efeito, lendo depois, achavam `None` e publicavam «o campo foi
            aceito e IGNORADO». Recusa nao lida vira ausencia publicada.
            """
            r = c.fala({"op": "criar_tabela", "database": "cmp", "tabela": tab,
                        "colunas": colunas,
                        "indices": indices or [
                            {"nome": "pk",
                             "colunas": ([{"coluna": 0}]
                                         if DEFEITO == "indice-velho"
                                         else [colunas[0]["nome"]]),
                             "unico": True}]})
            if exigir and not r.get("ok"):
                raise SystemExit(
                    f"MESA NAO POSTA: criar_tabela {tab} falhou -- "
                    f"{r.get('erro')}. Nenhuma sonda de efeito vale sobre "
                    "tabela que nao nasceu.")
            return r

        def corpo(r):
            """O que o servidor RESPONDEU, e nao o envelope que o carrega.

            A resposta e `{ok, op, resultado:{...}}`. Ler `r["linha"]` direto
            devolve `None` sempre -- e `None` aqui nao quer dizer «a coluna
            nasceu vazia», quer dizer «o medidor olhou no lugar errado». Foi o
            que aconteceu com CINCO sondas desta funcao antes de 07/09/2026,
            e uma delas chegou a publicar um controle disparando pelo motivo
            errado.
            """
            return r if DEFEITO == "envelope" else (r.get("resultado") or {})

        def linha_de(tab, rowid):
            """A linha, ja desembrulhada. UM leitor, para UM controle prova-lo.

            `ler` sem `com_versao` devolve a PROPRIA linha como resultado --
            nao um objeto com o campo `linha` dentro. Ter tres sondas lendo
            por conta propria foi o que deixou o erro passar em tres lugares
            de uma vez.
            """
            return corpo(c.fala({"op": "ler", "database": "cmp",
                                 "tabela": tab, "rowid": rowid}))

        # **O CONTROLE DO LEITOR**, antes de qualquer sonda usa-lo. Uma linha
        # que sabidamente existe tem de voltar com o valor que se gravou. Se
        # nem esta volta, o que as sondas abaixo chamariam de «campo ignorado»
        # e o leitor quebrado -- e um veredito de ausencia nasceria de um bug.
        cria("t_leitor", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "v", "tipo": "Int8"}])
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_leitor",
                "linha": {"id": 1, "v": 42}})
        prova = linha_de("t_leitor", 1)
        if prova.get("v") != 42:
            raise SystemExit(
                f"LEITOR QUEBRADO: gravei v=42 e li de volta {prova.get('v')!r}. "
                "Nenhuma sonda de efeito vale enquanto esta linha nao voltar 42.")

        # DEFAULT: cria com `padrao`, insere SEM a coluna, le de volta.
        cria("t_def", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                       {"nome": "v", "tipo": "Int8", "padrao": 7}])
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_def",
                "linha": {"id": 1}})
        veio = linha_de("t_def", 1).get("v")
        saida["default_de_coluna"] = (
            (TEM, "a linha nasceu com 7") if veio == 7 else
            (NAO, f"o campo `padrao` foi aceito e IGNORADO: v = {veio!r}"))

        # CHECK: cria com a restricao e tenta gravar o que ela proibiria.
        cria("t_ck", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "v", "tipo": "Int8", "check": "v > 0"}])
        proibido = c.fala({"op": "inserir", "database": "cmp", "tabela": "t_ck",
                           "linha": {"id": 1, "v": -5}})
        # **O CONTROLE.** Recusa nao prova restricao: a tabela pode nem existir
        # (se o campo `check` derrubou o `criar_tabela`), e ai TUDO e recusado.
        # So a recusa do proibido COM o permitido passando prova a restricao.
        permitido = c.fala({"op": "inserir", "database": "cmp", "tabela": "t_ck",
                            "linha": {"id": 2, "v": 5}})
        if not permitido.get("ok"):
            saida["check_constraint"] = (
                NAO, "veredito ANULADO pelo controle: o valor permitido tambem "
                     f"foi recusado ({(permitido.get('erro') or '')[:50]}) -- a "
                     "tabela nao nasceu")
        elif not proibido.get("ok"):
            saida["check_constraint"] = (TEM, "recusou -5 e aceitou 5")
        else:
            saida["check_constraint"] = (
                NAO, "o campo `check` foi aceito e IGNORADO: gravou v = -5")

        # CALCULADA: cria com a expressao e ve se o valor sai dela.
        cria("t_gc", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "a", "tipo": "Int8"},
                      {"nome": "b", "tipo": "Int8", "calculada": "a*2"}])
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_gc",
                "linha": {"id": 1, "a": 3}})
        veio = linha_de("t_gc", 1).get("b")
        saida["coluna_calculada"] = (
            (TEM, "b saiu 6") if veio == 6 else
            (NAO, f"o campo `calculada` foi aceito e IGNORADO: b = {veio!r}"))

        # INDICE PARCIAL: cria o indice com filtro, grava linha que o filtro
        # excluiria, e pergunta ao INDICE. Se ela aparece, o filtro nao existe.
        cria("t_ip", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "v", "tipo": "Int8"}],
             [{"nome": "pk", "colunas": ["id"], "unico": True},
              {"nome": "so_positivo", "colunas": ["v"], "onde": "v > 0"}],
             exigir=False)
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_ip",
                "linha": {"id": 1, "v": -5}})
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_ip",
                "linha": {"id": 2, "v": 7}})
        fora = c.fala({"op": "buscar", "database": "cmp", "tabela": "t_ip",
                       "indice": "so_positivo", "chave": [-5]})
        # **O CONTROLE.** Zero achado nao prova filtro: um indice que NAO
        # EXISTE tambem devolve zero. So o par -- a filtrada some E a incluida
        # aparece -- separa «filtrou» de «nao ha indice».
        dentro = c.fala({"op": "buscar", "database": "cmp", "tabela": "t_ip",
                         "indice": "so_positivo", "chave": [7]})
        n_fora = corpo(fora).get("encontrados", 0) if fora.get("ok") else None
        n_dentro = corpo(dentro).get("encontrados", 0) if dentro.get("ok") else None
        if n_dentro != 1:
            saida["indice_parcial"] = (
                NAO, "veredito ANULADO pelo controle: nem a linha INCLUIDA o "
                     f"indice devolveu (dentro={n_dentro!r}) -- o `onde` "
                     "derrubou a criacao ou o indice nao existe")
        elif n_fora == 0:
            saida["indice_parcial"] = (TEM, "guardou a incluida e nao a filtrada")
        else:
            saida["indice_parcial"] = (
                NAO, f"o campo `onde` foi aceito e IGNORADO: devolveu {n_fora}")

        # INDICE POR EXPRESSAO: o indice do protocolo aponta POSICAO de coluna;
        # nao ha onde escrever `lower(nome)`. A sonda tenta e a recusa e a prova.
        r = cria("t_ie", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(40)"}],
                 [{"nome": "pk", "colunas": ["id"], "unico": True},
                  {"nome": "por_baixo", "colunas": ["lower(nome)"]}],
                 exigir=False)
        # **O CONTROLE.** A recusa so prova a ausencia da EXPRESSAO se a mesma
        # tabela nascer com o indice escrito pelo nome da coluna. Sem ele, um
        # erro qualquer de digitacao viraria «nao tem indice por expressao».
        ctl = cria("t_ie2", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                             {"nome": "nome", "tipo": "Str(40)"}],
                   [{"nome": "pk", "colunas": ["id"], "unico": True},
                    {"nome": "por_nome", "colunas": ["nome"]}],
                   exigir=False)
        if not ctl.get("ok"):
            saida["indice_por_expressao"] = (
                NAO, "veredito ANULADO pelo controle: nem o indice por NOME de "
                     f"coluna nasceu ({(ctl.get('erro') or '')[:60]})")
        elif not r.get("ok"):
            saida["indice_por_expressao"] = (
                NAO, "o indice aceita NOME de coluna e recusa expressao: "
                     f"{(r.get('erro') or '')[:70]}")
        else:
            saida["indice_por_expressao"] = (MEIO, "criou -- conferir o que guardou")

        # VIEW: pergunta ao catalogo, que e a lista viva das operacoes.
        #
        # E aqui mora a MESMA armadilha que ja anulou dois vereditos deste
        # medidor, na terceira forma: lista VAZIA nao e ausencia, e sonda
        # quebrada. A primeira versao publicava «nenhuma operacao de visao
        # entre as 0» -- um «nao» certo pelo motivo errado, que e o pior tipo
        # de certo.
        #
        # E a CAUSA que eu diagnostiquei primeiro estava errada: culpei a
        # falta do `database` no pedido, e a prova real mostrou que sem ele o
        # servidor responde igual. O zero vinha do envelope lido no nivel
        # errado. Diagnostico plausivel nao e diagnostico medido -- e o errado
        # sobrevive melhor quando o conserto funcionou por outro motivo, que e
        # exatamente o que teria acontecido aqui.
        r = c.fala({"op": "catalogo", "database": "cmp"})
        nomes = set() if DEFEITO == "catalogo-vazio" else {
            o.get("nome") for o in (corpo(r).get("operacoes") or [])}
        if not nomes:
            raise SystemExit(
                "SONDA QUEBRADA: o catalogo devolveu ZERO operacao "
                f"({r.get('erro') or r}). Zero nao e ausencia de visao; e o "
                "medidor sem resposta. Conserte a sonda antes de publicar.")
        saida["view"] = ((TEM, f"ha operacao de visao entre as {len(nomes)}")
                         if {"criar_visao", "criar_view"} & nomes else
                         (NAO, f"nenhuma operacao de visao entre as {len(nomes)} "
                               "que o catalogo lista"))

        # O controle sai da MESMA conexao, senao ele provaria a saude de outra.
        r = c.fala({"op": "sql", "database": "cmp",
                    "sql": "CREATE ZZZZ nao_existe_de_proposito"})
        saida["__controle__"] = (TEM if r.get("ok") else NAO,
                                 (r.get("erro") or "aceitou")[:90])
        return saida
    finally:
        p.kill()
        p.wait()


# --------------------------------------------------------- os que sao CITADOS
#
# Nao ha motor nesta maquina, entao nao ha medida. O que cada um traz sai do
# documento que o leu, com a procedencia na propria celula.
CITACOES = {
    "cassandra": {
        "procedencia": "docs/CASSANDRA.md, fonte da 5.0.10 commit 7b5ab44, "
                       "lido em sessão anterior",
        "view": (NAO, "não há VIEW; há materialized view, com ressalva do próprio projeto"),
        "cte": (NAO, "CQL não tem subconsulta nem CTE"),
        "subconsulta": (NAO, "CQL não tem subconsulta"),
        "funcao_de_janela": (NAO, "CQL não tem função de janela"),
        "group_by": (MEIO, "GROUP BY só pelo prefixo da chave de partição"),
        "expressao_no_where": (NAO, "o WHERE do CQL é sobre a chave, sem expressão"),
        "indice_parcial": (NAO, "índice secundário existe; parcial não"),
        "indice_por_expressao": (NAO, "não há"),
        "check_constraint": (NAO, "não há"),
        "default_de_coluna": (NAO, "não há DEFAULT em CQL"),
        "coluna_calculada": (NAO, "não há"),
        "upsert": (TEM, "TODO INSERT é upsert: ele não lê antes de gravar"),
        "isolamento_acima_de_rc": (NAO, "não há transação multi-linha; há LWT por partição"),
        "trava_por_linha": (NAO, "não há trava: o conflito se resolve por carimbo de hora"),
        "tls_no_transporte": (TEM, "client_encryption_options no cassandra.yaml"),
        "direito_por_coluna": (NAO, "GRANT vai até a tabela"),
        "pitr": (TEM, "commitlog + snapshot; é o desenho dele"),
        "parametro_no_prepared": (TEM, "prepared statement com bind é o caminho normal"),
        "diff_de_dados": (MEIO, "reparo por árvore de Merkle diz o intervalo, não a linha"),
    },
    "hfsql": {
        "procedencia": "docs/HFSQL.md, folha de 2013-10 que NÃO está nesta sessão",
        "view": (TEM, "a folha lista visões"),
        "cte": (CITADO, "não apurado na folha"),
        "subconsulta": (CITADO, "não apurado na folha"),
        "funcao_de_janela": (CITADO, "não apurado na folha"),
        "group_by": (TEM, "SQL completo na folha"),
        "expressao_no_where": (TEM, "SQL completo na folha"),
        "indice_parcial": (TEM, "a folha lista índice parcial"),
        "indice_por_expressao": (CITADO, "não apurado"),
        "check_constraint": (CITADO, "não apurado"),
        "default_de_coluna": (TEM, "valor padrão no dicionário de dados"),
        "coluna_calculada": (TEM, "item calculado no dicionário"),
        "upsert": (CITADO, "não apurado"),
        "isolamento_acima_de_rc": (TEM, "a folha anuncia quatro níveis"),
        "trava_por_linha": (TEM, "trava por linha automática, na folha"),
        "tls_no_transporte": (TEM, "a folha anuncia canal cifrado"),
        "direito_por_coluna": (TEM, "direito por coluna na folha"),
        "pitr": (CITADO, "não apurado"),
        "parametro_no_prepared": (TEM, "consulta parametrizada é o normal do WLangage"),
        "diff_de_dados": (TEM, "WDHFDiff compara estrutura e dados"),
    },
}


def compila_o_nosso():
    """Recompila o `phxsqld` ANTES de medir, e nao depois de duvidar.

    Petrea desta casa: *medidor com binario velho mede o passado.* A primeira
    corrida de 07/09/2026 subiu um binario anterior ao `procurar_texto` e
    publicou «118 operacoes» quando o catalogo tinha 123 -- o veredito nao
    mudava, mas a EVIDENCIA dele era de outro dia.

    O `flock` e o mesmo de toda compilacao aqui: duas frentes compilando ao
    mesmo tempo derrubam uma a outra.
    """
    r = subprocess.run(["flock", "/tmp/phx-cargo.lock", "cargo", "build",
                        "--release", "-p", "phxsql-server"],
                       cwd=RAIZ, capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("nao compilou o phxsqld:\n" + r.stderr[-2000:])


def main():
    perguntas = PERGUNTAS
    print(f"O comparativo medido — {len(perguntas)} perguntas por SQL, "
          f"{len(sonda_codigo())} por sonda de código\n")
    compila_o_nosso()
    print("  phxsqld       recompilado antes de medir")

    motores = {}
    motores["sqlite"] = por_sqlite(perguntas)
    print(f"  SQLite(R)     {sqlite3.sqlite_version}: respondeu")

    if prepara_mysql():
        motores["mysql"] = por_processo(perguntas, "mysql", mysql_roda)
        print("  MySQL(R)      respondeu")
    else:
        print("  MySQL(R)      NAO SUBIU -- as celulas dele saem 'sem-motor'")
        motores["mysql"] = {c: (SEM, "motor fora do ar") for c, _t, _s in perguntas}

    if prepara_pg():
        motores["postgres"] = por_processo(perguntas, "postgres", pg_roda)
        print("  PostgreSQL(R) respondeu")
    else:
        print("  PostgreSQL(R) NAO SUBIU -- as celulas dele saem 'sem-motor'")
        motores["postgres"] = {c: (SEM, "motor fora do ar") for c, _t, _s in perguntas}

    with tempfile.TemporaryDirectory(prefix="phx-cmp-") as base:
        motores["phxsql"] = por_phxsql(perguntas, base)
    ctl_phx = motores["phxsql"].pop("__controle__")
    print("  PhxSql        respondeu\n")

    # **O portao do medidor: um CONTROLE que tem de FALHAR.**
    #
    # A primeira versao deste portao exigia que o motor respondesse coisas
    # diferentes -- e reprovou o PostgreSQL(R), que respondeu `tem` as treze
    # porque ELE TEM AS TREZE. Resposta uniforme nao e cliente quebrado quando
    # o motor e completo; o portao estava medindo a diversidade da resposta em
    # vez da saude do cliente.
    #
    # O certo e o controle positivo que esta casa ja usa em toda bancada: uma
    # instrucao que NENHUM motor pode aceitar. Se um aceitar, o `ok` nao esta
    # vindo do motor -- esta vindo de um cliente que engole erro.
    CONTROLE = "CREATE ZZZZ nao_existe_de_proposito"
    controles = {
        "sqlite": por_sqlite([("ctl", "", {"sqlite": CONTROLE})])["ctl"],
        "mysql": por_processo([("ctl", "", {"mysql": CONTROLE})], "mysql", mysql_roda)["ctl"],
        "postgres": por_processo([("ctl", "", {"postgres": CONTROLE})], "postgres", pg_roda)["ctl"],
        "phxsql": ctl_phx,
    }
    for nome, (ver, msg) in controles.items():
        assert ver == NAO, (
            f"o CONTROLE passou em {nome} ({msg!r}): uma instrucao invalida foi "
            "aceita, entao o `ok` nao vem do motor e nenhuma celula desta "
            "coluna vale")

    codigo = sonda_codigo()
    linhas = []
    for chave, titulo, _s in perguntas:
        linhas.append({
            "chave": chave, "titulo": titulo, "como": "SQL no motor vivo",
            "phxsql": motores["phxsql"][chave],
            "mysql": motores["mysql"][chave],
            "postgres": motores["postgres"][chave],
            "sqlite": motores["sqlite"][chave],
            "cassandra": CITACOES["cassandra"].get(chave, (CITADO, "não apurado")),
            "hfsql": CITACOES["hfsql"].get(chave, (CITADO, "não apurado")),
        })
    for chave, d in codigo.items():
        linhas.append({
            "chave": chave, "titulo": d["titulo"], "como": "sonda de código",
            "nota": d.get("nota", ""),
            "phxsql": d["phxsql"],
            "mysql": (CITADO, "não perguntado por SQL"),
            "postgres": (CITADO, "não perguntado por SQL"),
            "sqlite": (CITADO, "não perguntado por SQL"),
            "cassandra": CITACOES["cassandra"].get(chave, (CITADO, "não apurado")),
            "hfsql": CITACOES["hfsql"].get(chave, (CITADO, "não apurado")),
        })

    faltam = [l for l in linhas if l["phxsql"][0] in (NAO, MEIO)]
    saida = {
        "quando": datetime.datetime.now().isoformat(timespec="seconds"),
        "motores_vivos": {
            "sqlite": sqlite3.sqlite_version,
            "mysql": subprocess.run(["mysql", "-N", "-B", "-e", "select version()"],
                                    capture_output=True, text=True).stdout.strip() or "fora do ar",
            "postgres": subprocess.run(
                ["sudo", "-u", "postgres", "psql", "-tAc", "show server_version"],
                capture_output=True, text=True).stdout.strip() or "fora do ar",
            "phxsql": subprocess.run([str(RAIZ / "target/release/phxsqld"), "--version"],
                                     capture_output=True, text=True).stdout.strip() or "?",
        },
        "sem_motor_nesta_maquina": {
            "cassandra": CITACOES["cassandra"]["procedencia"],
            "hfsql": CITACOES["hfsql"]["procedencia"],
        },
        "capacidades": len(linhas),
        "faltam_no_phxsql": len(faltam),
        "linhas": linhas,
    }
    ALVO.write_text(json.dumps(saida, indent=2, ensure_ascii=False) + "\n")

    print(f"{'capacidade':38} {'phx':6} {'my':6} {'pg':6} {'lite':6}")
    print("-" * 68)
    for l in linhas:
        print(f"{l['titulo'][:37]:38} {l['phxsql'][0]:6} {l['mysql'][0]:6} "
              f"{l['postgres'][0]:6} {l['sqlite'][0]:6}")
    print(f"\n{len(faltam)} de {len(linhas)} capacidades faltam ou estão pela metade no PhxSql")
    print(f"gravado: {ALVO}")


if __name__ == "__main__":
    main()
