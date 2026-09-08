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

**Sonda de codigo**, so onde SQL nao alcanca e a resposta mora na LEITURA do
fonte (trava, TLS). Cada uma aponta arquivo e linha, e o texto do veredito diz
o que a sonda olhou.

**Sonda VIVA**, para PITR, direito por coluna, parametro (`?`) e diferencas de
dados: nenhuma delas se prova por grep -- cada uma sobe o servidor de verdade
(a de coluna e a de PITR, um `phxsqld` PROPRIO, porque precisam de um
cadastro ou de uma `replicacao` que o servidor principal deste medidor nao
tem) e mede o EFEITO, com o CONTROLE na mesma corrida. Ate 08/09/2026 as
quatro eram sonda de codigo com veredito CRAVADO -- e cravar um `nao` que virou
`tem` no motor e o mesmo erro que a celula da trava por linha ja pagou.

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
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ALVO = AQUI / "resultados.json"
PORTA = 7731
PHXSQLD_BIN = RAIZ / "target" / "release" / "phxsqld"

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

    # As QUATRO sondas que moravam aqui ate 08/09/2026 -- direito por coluna,
    # PITR, parametro (`?`) e diferencas de dados -- viraram sonda VIVA:
    # `sonda_direito_coluna()`, `sonda_pitr()`, e os dois probes dentro de
    # `por_phxsql()` (parametro_no_prepared, diff_de_dados). Grep prova que um
    # trecho existe, nunca que o EFEITO acontece -- e a diferenca era real: as
    # quatro estavam cravadas em NAO/MEIO com o motor ja respondendo.

    return fora


# --------------------------------------------------------- servidor PROPRIO
#
# `por_phxsql()` sobe UM `phxsqld` com o cadastro padrao (adm com tudo). Duas
# capacidades precisam de um servidor DIFERENTE -- coluna precisa de um
# cadastro com regra por coluna, e PITR precisa de `replicacao.imagem_da_linha`
# ligada ANTES de subir -- entao as duas sondas abrem o SEU PROPRIO processo,
# fora de `por_phxsql()`, e nunca derrubam por pkill: so pelo PID que
# guardaram.
def _sobe(base, cfg):
    os.makedirs(base, exist_ok=True)
    with open(os.path.join(base, "config.json"), "w") as f:
        json.dump(cfg, f)
    log = open(os.path.join(base, "servidor.log"), "a")
    p = subprocess.Popen([str(PHXSQLD_BIN)], cwd=base, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    porta = int(cfg["bind"].rsplit(":", 1)[1])
    fim = time.monotonic() + 25
    while True:
        try:
            socket.create_connection(("127.0.0.1", porta), 0.3).close()
            return p
        except OSError:
            if p.poll() is not None or time.monotonic() > fim:
                p.kill()
                raise SystemExit(
                    f"o servidor proprio nao subiu na porta {porta}:\n"
                    + open(os.path.join(base, "servidor.log")).read())
            time.sleep(0.1)


def _derruba(p, base):
    try:
        p.send_signal(signal.SIGTERM)
        p.wait(timeout=8)
    except Exception:
        try:
            p.kill()
        except Exception:
            pass
    shutil.rmtree(base, ignore_errors=True)


class _Fala:
    """Uma conexao crua, json-por-linha -- para servidores com cadastro
    proprio que o `oficina.Conexao` (login fixo do adm) nao serve, porque
    aqui e a IDENTIDADE de quem conecta que esta sob prova."""

    def __init__(self, porta, token):
        self.s = socket.create_connection(("127.0.0.1", porta), timeout=10)
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.f = self.s.makefile("rwb")
        self.token = token

    def __call__(self, **pedido):
        pedido.setdefault("token", self.token)
        self.f.write((json.dumps(pedido) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise ConnectionError("o servidor fechou a conexao")
        return json.loads(linha.decode())

    def fechar(self):
        for c in (self.f, self.s):
            try:
                c.close()
            except OSError:
                pass


def _hash(senha):
    """`phxsqld --senha`, o mesmo caminho que `oficina.hash_da_senha` usa --
    aqui direto, sem depender do sys.path que `por_phxsql()` arruma."""
    r = subprocess.run([str(PHXSQLD_BIN), "--senha"], input=senha + "\n",
                       capture_output=True, text=True)
    return r.stdout.split('": "')[1].split('"')[0]


def sonda_direito_coluna():
    """Servidor PROPRIO com um cadastro de direito por COLUNA, e o EFEITO
    medido: o `varrer` de quem tem `salario` negado volta SEM a coluna, e o
    MESMO `varrer` por quem NAO tem a regra (o controle) volta COM ela.

    O molde do cadastro e o mesmo do teste
    `testes_direito_por_coluna::so_a_folha_tem_regra` em `servidor.rs`, e do
    MANUAL 14.3.2: a base `"*"` com a tabela `"folha"` carregando `"colunas"`.
    """
    regra = {"ler": True, "inserir": True, "alterar": True, "excluir": True,
             "criar": True, "diario": True, "administrar": True,
             "replicar": True, "verificar": True, "reindexar": True}
    token = "prova-coluna"
    cfg = {
        "base": "base", "bind": f"127.0.0.1:{PORTA + 1}", "token": token,
        "usuarios": [
            {"login": "bea", "nome": "Bea (controle, sem regra)", "id": 1,
             "senha_hash": _hash("senha-da-bea"), "bases": {"*": dict(regra)}},
            {"login": "ana", "nome": "Ana (salario negado)", "id": 2,
             "senha_hash": _hash("senha-da-ana"),
             "bases": {"*": {**regra, "tabelas": {"folha": {
                 **regra,
                 "colunas": {"salario": {"ler": False, "alterar": False}}}}}}},
        ],
    }
    base = tempfile.mkdtemp(prefix="phx-cmp-coluna-")
    p = _sobe(base, cfg)
    try:
        bea = _Fala(PORTA + 1, token)
        if not bea(op="login", usuario="bea", senha="senha-da-bea").get("ok"):
            raise SystemExit("MESA NAO POSTA: bea nao logou")
        if not bea(op="criar_database", database="b").get("ok"):
            raise SystemExit("MESA NAO POSTA: database b nao nasceu")
        r = bea(op="criar_tabela", database="b", tabela="folha",
                colunas=[{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                         {"nome": "nome", "tipo": "Str(20)"},
                         {"nome": "salario", "tipo": "Int4"}],
                indices=[{"nome": "porId", "colunas": ["id"], "unico": True,
                          "primario": True}])
        if not r.get("ok"):
            raise SystemExit(f"MESA NAO POSTA: folha nao nasceu -- {r.get('erro')}")
        r = bea(op="inserir", database="b", tabela="folha",
                linha={"id": 1, "nome": "ana", "salario": 5000})
        if not r.get("ok"):
            raise SystemExit(f"MESA NAO POSTA: a linha nao gravou -- {r.get('erro')}")

        linha_bea = (bea(op="varrer", database="b", tabela="folha")
                    .get("resultado") or {}).get("linhas", [{}])[0]

        ana = _Fala(PORTA + 1, token)
        if not ana(op="login", usuario="ana", senha="senha-da-ana").get("ok"):
            raise SystemExit("MESA NAO POSTA: ana nao logou")
        linha_ana = (ana(op="varrer", database="b", tabela="folha")
                    .get("resultado") or {}).get("linhas", [{}])[0]
        ana.fechar()
        bea.fechar()

        # **O CONTROLE.** So a coluna sumir NA RESTRITA e continuar no
        # controle prova a regra -- se as duas viessem sem ela, o dado nunca
        # teria sido gravado; se as duas viessem com ela, a regra nao vale
        # nada. E' a mesma disciplina das outras sondas de efeito deste
        # medidor: recusa (ou ausencia) sem controle nao prova nada sozinha.
        if "salario" not in linha_bea:
            veredito = (NAO, "veredito ANULADO pelo controle: nem quem NAO "
                             f"tem regra de coluna viu `salario` ({linha_bea!r})")
        elif "salario" not in linha_ana:
            veredito = (TEM, "o varrer de ana (salario negado) veio SEM a "
                             "coluna; o de bea (controle, mesmo cadastro sem "
                             "`colunas`) veio COM ela")
        else:
            veredito = (NAO, f"a coluna `salario` vazou para ana: {linha_ana!r}")
    finally:
        _derruba(p, base)

    return {
        "titulo": "Direito por COLUNA",
        "phxsql": veredito,
        "nota": "o direito por TABELA existe desde o pedido 124; o cadastro "
                "e o molde de `testes_direito_por_coluna` em servidor.rs e do "
                "MANUAL 14.3.2",
    }


def sonda_pitr():
    """Servidor PROPRIO com `replicacao.imagem_da_linha` ligada (sem ela nao
    ha PITR: o evento nao carrega a linha, e o `restaurar_backup` recusa
    nomeando o interruptor), e o EFEITO de uma restauracao a um INSTANTE.

    A sequencia e a de `docs/RESTAURACAO.md` § 7.10 -- REAPROVEITADA, nao
    copiada por inteiro: aqui e um veredito so (TEM/NAO), nao as 22
    conferencias de `bancada/pitr/provar.py` (essa continua sendo a prova
    funda; esta e a que alimenta a tabela).
    """
    token = "prova-pitr"
    base = tempfile.mkdtemp(prefix="phx-cmp-pitr-")
    cfg = {
        "base": "base", "bind": f"127.0.0.1:{PORTA + 2}", "token": token,
        "replicacao": {"papel": "isolado", "imagem_da_linha": True},
        "backup": {"destino": os.path.join(base, "backup")},
    }
    p = _sobe(base, cfg)
    try:
        f = _Fala(PORTA + 2, token)

        def corpo(r):
            return r.get("resultado", r)

        r = f(op="criar_database", database="loja")
        if not r.get("ok"):
            raise SystemExit(f"MESA NAO POSTA: database loja nao nasceu -- {r.get('erro')}")
        r = f(op="criar_tabela", database="loja", tabela="clientes",
              colunas=[{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                       {"nome": "nome", "tipo": "Str(20)"}],
              indices=[{"nome": "porId", "colunas": ["id"], "unico": True,
                        "primario": True}])
        if not r.get("ok"):
            raise SystemExit(f"MESA NAO POSTA: clientes nao nasceu -- {r.get('erro')}")

        # A historia: linha 1, COPIA, linha 2, alteracao da 1, linha 3 -- e o
        # CORTE fica entre a alteracao e a linha 3. O relogio precisa ANDAR
        # entre cada passo, senao tres escritas caem no mesmo milissegundo e
        # nao existe instante entre elas (a mesma armadilha que
        # `bancada/pitr/provar.py` ja paga).
        f(op="inserir", database="loja", tabela="clientes", linha={"id": 1, "nome": "um"})
        time.sleep(0.01)
        r = f(op="backup", destino=cfg["backup"]["destino"], database="loja", zip=True)
        copia = corpo(r).get("arquivo")
        if not copia:
            raise SystemExit(f"MESA NAO POSTA: o backup nao saiu -- {r.get('erro')}")
        time.sleep(0.01)
        f(op="inserir", database="loja", tabela="clientes", linha={"id": 2, "nome": "dois"})
        time.sleep(0.01)
        f(op="atualizar", database="loja", tabela="clientes", rowid=1,
          linha={"id": 1, "nome": "um alterado"})
        time.sleep(0.01)
        f(op="inserir", database="loja", tabela="clientes", linha={"id": 3, "nome": "tres"})

        # O CORTE sai do PROPRIO diario, medido -- nunca digitado. E' o mesmo
        # erro que esta casa ja pagou quatro vezes com numero de painel.
        eventos = corpo(f(op="diario", database="loja", tabela="clientes", max=100))["eventos"]
        if len(eventos) != 4:
            raise SystemExit(f"PITR SEM DIARIO: esperava 4 eventos, veio {len(eventos)}")
        carimbos = [e["carimbo_ms"] for e in eventos]
        corte = carimbos[3] - 1
        if corte < carimbos[2]:
            raise SystemExit("PITR SEM CORTE: o relogio nao andou entre a "
                             f"alteracao e a terceira linha ({carimbos})")

        r = f(op="restaurar_backup", origem=copia, database="loja_no_meio", ate_ms=corte)
        pitr = corpo(r).get("pitr")
        linhas = [(l["id"], l["nome"]) for l in
                  corpo(f(op="varrer", database="loja_no_meio", tabela="clientes"))
                  .get("linhas", [])]

        # **O CONTROLE.** O MESMO backup, SEM `ate`, tem de voltar SO com a
        # linha 1 -- sem ele uma restauracao que devolvesse duas linhas por
        # acaso passaria como se tivesse cortado certo.
        r2 = f(op="restaurar_backup", origem=copia, database="loja_da_copia")
        ctl = [(l["id"], l["nome"]) for l in
               corpo(f(op="varrer", database="loja_da_copia", tabela="clientes"))
               .get("linhas", [])]
        controle_ok = "pitr" not in corpo(r2) and ctl == [(1, "um")]

        if not controle_ok:
            veredito = (NAO, "veredito ANULADO pelo controle: o MESMO backup "
                             f"SEM `ate` nao voltou so com a linha 1 ({ctl!r})")
        elif pitr is not None and linhas == [(1, "um alterado"), (2, "dois")]:
            veredito = (TEM, f"`ate`=corte devolveu {linhas!r} (a 1 alterada, "
                             f"a 2, nada da 3); reaplicados="
                             f"{pitr.get('reaplicados')}, pulados="
                             f"{pitr.get('tabelas', [{}])[0].get('pulados')}")
        else:
            veredito = (NAO, f"o EFEITO nao bateu: {linhas!r} (esperava a 1 "
                             "alterada, a 2 e nada da 3)")
    finally:
        _derruba(p, base)

    return {
        "titulo": "Recuperação a um ponto no tempo (PITR)",
        "phxsql": veredito,
        "nota": "sequência de docs/RESTAURACAO.md § 7.10 -- a mesma que "
                "bancada/pitr/provar.py roda com 22 conferências",
    }


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
        # Uma linha em `c`, para a sonda de EFEITO da visao (§ abaixo) ter o
        # que comparar -- as treze perguntas de SQL testam se a INSTRUCAO
        # passa, e nao encostam no dado.
        c.fala({"op": "inserir", "database": "cmp", "tabela": "c",
                "linha": {"id": 1, "nome": "um", "cidade": "Blumenau",
                          "preco": "10.00"}})
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

        # O EFEITO da visao, acrescentado em 08/09/2026: o catalogo listar a
        # OPERACAO nao prova que `CREATE VIEW` pela op `sql` funciona nem que
        # ler dela devolve o dado certo -- so prova que o verbo existe. So
        # tenta o SQL quando o catalogo ja disse TEM: sem a operacao, o SQL
        # abaixo recusaria por op ausente, e isso nao acrescentaria nada ao
        # veredito que o catalogo ja deu.
        if saida["view"][0] == TEM:
            linhas_c = corpo(c.fala({"op": "varrer", "database": "cmp",
                                     "tabela": "c"})).get("linhas") or []
            cv = c.fala({"op": "sql", "database": "cmp",
                        "sql": "CREATE VIEW v_c AS SELECT * FROM c"})
            sv = c.fala({"op": "sql", "database": "cmp", "sql": "SELECT * FROM v_c"})
            linhas_v = corpo(sv).get("linhas") if sv.get("ok") else None
            if cv.get("ok") and sv.get("ok") and linhas_c and linhas_v == linhas_c:
                saida["view"] = (
                    TEM, f"CREATE VIEW v_c AS SELECT * FROM c e SELECT * FROM "
                         f"v_c devolveram as {len(linhas_c)} linha(s) de `c`")
            else:
                saida["view"] = (
                    NAO, "catálogo lista `criar_visao`, mas o EFEITO falhou: "
                         f"CREATE VIEW -> {cv.get('erro') or 'ok'}; "
                         f"SELECT * FROM v_c -> {sv.get('erro') or linhas_v!r}")

        # PARAMETRO EM INSTRUCAO PREPARADA (`?`): grava um valor conhecido,
        # busca por `?` com `parametros`, e o CONTROLE e o MESMO `?` SEM
        # `parametros` -- que tem de RECUSAR nomeando quantos vieram e
        # quantos faltam. So a recusa do sem-parametro ao lado do achado do
        # com-parametro prova que quem respondeu foi o parametro, e nao
        # coincidencia de `id = ?` sempre passar.
        cria("t_param", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                         {"nome": "nome", "tipo": "Str(20)"}])
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_param",
                "linha": {"id": 1, "nome": "um"}})
        com_param = c.fala({"op": "sql", "database": "cmp",
                            "sql": "SELECT * FROM t_param WHERE id = ?",
                            "parametros": [1]})
        linhas_p = corpo(com_param).get("linhas") if com_param.get("ok") else None
        sem_param = c.fala({"op": "sql", "database": "cmp",
                            "sql": "SELECT * FROM t_param WHERE id = ?"})
        if sem_param.get("ok"):
            saida["parametro_no_prepared"] = (
                NAO, "veredito ANULADO pelo controle: o `?` SEM `parametros` "
                     f"foi ACEITO ({corpo(sem_param)!r}) -- o motor nao esta "
                     "amarrando o `?` ao parametro")
        elif not linhas_p or len(linhas_p) != 1 or linhas_p[0].get("id") != 1:
            saida["parametro_no_prepared"] = (
                NAO, f"com `parametros:[1]` o motor nao devolveu a linha 1: "
                     f"{(com_param.get('erro') or linhas_p)!r}")
        else:
            saida["parametro_no_prepared"] = (
                TEM, "`WHERE id = ?` com `parametros:[1]` devolveu a linha 1; "
                     f"sem `parametros` recusou: "
                     f"{(sem_param.get('erro') or '')[:70]}")

        # DIFERENCAS: duas tabelas com a MESMA chave unica e uma linha
        # diferente; `diferencas` tem de NOMEAR a chave e a coluna. O
        # CONTROLE e o par IGUAL, que tem de vir com `diferentes` vazio e
        # `iguais == 1` -- sem ele, um bug que marcasse tudo como diferente
        # passaria junto.
        cria("t_diff_a", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(20)"}])
        cria("t_diff_b", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(20)"}])
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_diff_a",
                "linha": {"id": 1, "nome": "um"}})
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_diff_b",
                "linha": {"id": 1, "nome": "dois"}})
        r_dif = c.fala({"op": "diferencas", "database": "cmp", "a": "t_diff_a",
                        "b": "t_diff_b", "indice": "pk"})
        dif = corpo(r_dif).get("diferentes") if r_dif.get("ok") else None

        cria("t_diff_c", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(20)"}])
        cria("t_diff_d", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(20)"}])
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_diff_c",
                "linha": {"id": 1, "nome": "um"}})
        c.fala({"op": "inserir", "database": "cmp", "tabela": "t_diff_d",
                "linha": {"id": 1, "nome": "um"}})
        r_igual = c.fala({"op": "diferencas", "database": "cmp", "a": "t_diff_c",
                          "b": "t_diff_d", "indice": "pk"})
        igual = corpo(r_igual) if r_igual.get("ok") else None

        controle_diff_ok = (bool(igual) and igual.get("diferentes") == []
                            and igual.get("iguais") == 1)
        if not controle_diff_ok:
            saida["diff_de_dados"] = (
                NAO, "veredito ANULADO pelo controle: o par IGUAL nao voltou "
                     f"`diferentes: []` e `iguais: 1` ({igual!r})")
        elif not dif:
            saida["diff_de_dados"] = (
                NAO, f"`diferencas` nao apontou a linha diferente: "
                     f"{(r_dif.get('erro') or dif)!r}")
        elif dif[0].get("chave") == [1] and dif[0].get("colunas") == ["nome"]:
            saida["diff_de_dados"] = (
                TEM, f"`diferencas` nomeou a chave [1] e a coluna `nome`: {dif}")
        else:
            saida["diff_de_dados"] = (
                MEIO, f"achou diferenca, mas fora do formato esperado: {dif}")

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

    # As quatro sondas VIVAS: cada uma sobe o efeito, nao o grep. As duas que
    # precisam de servidor PROPRIO (cadastro de coluna; `replicacao` ligada
    # antes do arranque) entram por chamada direta; as outras duas ja foram
    # medidas dentro de `por_phxsql()`, e so precisam do titulo e da nota.
    codigo["direito_por_coluna"] = sonda_direito_coluna()
    print("  direito_coluna  medido (servidor proprio, ana x bea)")
    codigo["pitr"] = sonda_pitr()
    print("  pitr            medido (servidor proprio, replicacao ligada)")
    codigo["parametro_no_prepared"] = {
        "titulo": "Parâmetro em instrução preparada (`?`)",
        "phxsql": motores["phxsql"]["parametro_no_prepared"],
        "nota": "o driver ODBC já liga e manda `parametros` desde o "
                "`SQLBindParameter`; a promoção que faltava era o servidor "
                "ler `parametros` na op `sql`, medida aqui pelo soquete",
    }
    codigo["diff_de_dados"] = {
        "titulo": "Dizer ONDE duas tabelas diferem",
        "phxsql": motores["phxsql"]["diff_de_dados"],
        "nota": "o `checksum` dizia SE diferem; a op `diferencas` é o "
                "terceiro irmão da conferência própria, junto de `juntar` e "
                "`unir`",
    }

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
