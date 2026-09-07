#!/usr/bin/env python3
"""Sonda dos GAPS de SQL: manda cada comando ao motor vivo e cola a recusa.

    python3 bancada/gaps-sql/sondar.py            # tudo
    python3 bancada/gaps-sql/sondar.py postgresql # so um motor
    PHX_GAPS_PORTA=6110 python3 bancada/gaps-sql/sondar.py

Existe por causa de uma lei da casa: *lista de gap sem a recusa colada e
palpite*. As respostas B, C, D, O, P e Q do PDF das 26 perguntas afirmam que
tal comando do PostgreSQL(R), do MariaDB(R), do MySQL(R), do SQLite(R) ou do
Cassandra(R) nao existe aqui — e cada uma dessas afirmacoes tem de sair de uma
corrida contra o `phxsqld`, nunca da leitura de `docs/SQL.md`.

# O instrumento antes do veredito

O primeiro bloco da sonda e o CONTROLE POSITIVO: `SELECT`, `SELECT COUNT(*)`,
`BEGIN`/`COMMIT`, `CREATE TRIGGER`, `CALL`, `SHOW TRIGGERS`. Se ele nao passar
inteiro, o zero dos outros blocos nao vale nada — seria a sonda medindo a
propria conexao quebrada, e nao o motor. A sonda PARA quando o controle falha.

# O que ela mede, e o que ela nao mede

Ela mede uma coisa so: **o motor aceita ou recusa este texto SQL?** Ela nao
julga se o gap importa (isso e das respostas em `docs/pdf/respostas/`) e nao
mede desempenho. Um comando pode ser recusado por tres motivos diferentes —
sintaxe desconhecida, clausula sem substrato, ou falta de indice — e a sonda
cola a mensagem inteira justamente porque os tres nao sao a mesma coisa.
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import time

PORTA = int(os.environ.get("PHX_GAPS_PORTA", "6110"))
BASE = f"/tmp/phx-f2-{os.getpid()}"
BINARIO = "target/release/phxsqld"
BANCO = "loja"


class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer."""

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        shutil.rmtree(BASE, ignore_errors=True)
        os.makedirs(BASE + "/dados", exist_ok=True)
        cfg = BASE + "/config.json"
        with open(cfg, "w") as f:
            json.dump(
                {
                    "bind": f"127.0.0.1:{PORTA}",
                    "base": BASE + "/dados",
                    "token": "t",
                    "web": {"ligado": False},
                },
                f,
            )
        self.p = subprocess.Popen(
            [BINARIO, "--config", cfg],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor nao subiu na porta {PORTA}")
        self.s = socket.create_connection(("127.0.0.1", PORTA), 15)
        self.f = self.s.makefile("rwb")
        return self

    def __exit__(self, *_):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass
        self.p.terminate()
        self.p.wait(timeout=10)
        shutil.rmtree(BASE, ignore_errors=True)

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def sql(self, texto):
        return self.pedir(op="sql", database=BANCO, texto=texto)


def preparar(sv):
    """Uma base com o minimo que um cadastro tem: chave, indice e tres linhas."""
    sv.pedir(op="criar_database", database=BANCO)
    sv.pedir(
        op="criar_tabela",
        database=BANCO,
        tabela="clientes",
        colunas=[
            {"nome": "id", "tipo": "Sequence", "obrigatoria": True},
            {"nome": "nome", "tipo": "Str(60)"},
            {"nome": "cidade", "tipo": "Str(40)"},
            {"nome": "saldo", "tipo": "Decimal(12,2)"},
        ],
        indices=[
            {"nome": "porId", "colunas": ["id"], "unico": True, "primario": True},
            {"nome": "porNome", "colunas": ["nome"]},
        ],
    )
    # A tabela irma de chave `Int8`: o controle positivo do `WHERE` por chave
    # precisa dela porque a de chave `Sequence` RECUSA (ver o achado no
    # bloco `achados`), e um controle que falha nao prova nada.
    sv.pedir(
        op="criar_tabela",
        database=BANCO,
        tabela="itens",
        colunas=[
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "descricao", "tipo": "Str(30)"},
        ],
        indices=[{"nome": "porId", "colunas": ["id"], "unico": True, "primario": True}],
    )
    sv.pedir(op="inserir", database=BANCO, tabela="itens", valores={"id": 1, "descricao": "cafe"})
    # A tabela com indice de TEXTO (.fts): o `MATCH ... AGAINST` do MariaDB(R)
    # e o `fts5` do SQLite(R) so podem ser chamados de gap depois de a sonda
    # tentar o caminho que existe aqui.
    sv.pedir(
        op="criar_tabela",
        database=BANCO,
        tabela="chamados",
        colunas=[
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "corpo", "tipo": "Str(200)"},
        ],
        indices=[{"nome": "porId", "colunas": ["id"], "unico": True, "primario": True}],
        indices_texto=[{"nome": "porCorpo", "coluna": "corpo"}],
    )
    sv.pedir(op="inserir", database=BANCO, tabela="chamados",
             valores={"id": 1, "corpo": "a fenix renasce das cinzas"})
    sv.pedir(
        op="inserir_lote",
        database=BANCO,
        tabela="clientes",
        linhas=[
            {"nome": "Adriano", "cidade": "Blumenau", "saldo": "10.00"},
            {"nome": "Beatriz", "cidade": "Joinville", "saldo": "20.50"},
            {"nome": "Carlos", "cidade": "Blumenau", "saldo": "30.25"},
        ],
    )


# --------------------------------------------------------------- o controle
#
# Seis comandos que TEM de passar. Nao sao enfeite: sem eles, um "recusado"
# nos blocos de baixo poderia ser a conexao, o token ou a base — e nao o motor.
CONTROLE = [
    "SELECT * FROM clientes",
    "SELECT COUNT(*) FROM clientes",
    "SELECT nome AS quem FROM clientes ORDER BY nome LIMIT 2",
    "SELECT * FROM clientes WHERE nome = 'Adriano'",
    "SELECT * FROM itens WHERE id = 1",
    "BEGIN",
    "COMMIT",
    "SHOW TRIGGERS",
    "SHOW PROCEDURES",
]

# ------------------------------------------------------------- os candidatos
#
# (motor, rotulo, sql). O rotulo e o nome do comando no manual de origem, para
# que a resposta possa citar «ALTER TABLE … RENAME» sem repetir o SQL inteiro.
CANDIDATOS = [
    # ---------------------------------------------------------- PostgreSQL(R)
    ("postgresql", "INSERT", "INSERT INTO clientes (nome) VALUES ('Zeca')"),
    ("postgresql", "UPDATE", "UPDATE clientes SET cidade = 'Itajai' WHERE id = 1"),
    ("postgresql", "DELETE", "DELETE FROM clientes WHERE id = 1"),
    ("postgresql", "CREATE TABLE", "CREATE TABLE fornecedores (id integer, nome text)"),
    ("postgresql", "DROP TABLE", "DROP TABLE clientes"),
    ("postgresql", "ALTER TABLE ADD COLUMN", "ALTER TABLE clientes ADD COLUMN uf char(2)"),
    ("postgresql", "CREATE INDEX", "CREATE INDEX porCidade ON clientes (cidade)"),
    ("postgresql", "DROP INDEX", "DROP INDEX porNome"),
    ("postgresql", "CREATE VIEW", "CREATE VIEW vw AS SELECT * FROM clientes"),
    ("postgresql", "TRUNCATE", "TRUNCATE TABLE clientes"),
    ("postgresql", "GRANT", "GRANT SELECT ON clientes TO leitor"),
    ("postgresql", "REVOKE", "REVOKE SELECT ON clientes FROM leitor"),
    ("postgresql", "CREATE ROLE", "CREATE ROLE leitor"),
    ("postgresql", "EXPLAIN", "EXPLAIN SELECT * FROM clientes"),
    ("postgresql", "WHERE AND", "SELECT * FROM clientes WHERE id = 1 AND cidade = 'Blumenau'"),
    ("postgresql", "WHERE OR", "SELECT * FROM clientes WHERE id = 1 OR id = 2"),
    ("postgresql", "WHERE IN", "SELECT * FROM clientes WHERE id IN (1, 2)"),
    ("postgresql", "WHERE BETWEEN", "SELECT * FROM clientes WHERE id BETWEEN 1 AND 2"),
    ("postgresql", "WHERE LIKE", "SELECT * FROM clientes WHERE nome LIKE 'A%'"),
    ("postgresql", "WHERE IS NULL", "SELECT * FROM clientes WHERE cidade IS NULL"),
    ("postgresql", "WHERE sem indice", "SELECT * FROM clientes WHERE cidade = 'Blumenau'"),
    ("postgresql", "GROUP BY", "SELECT cidade, COUNT(*) FROM clientes GROUP BY cidade"),
    ("postgresql", "HAVING", "SELECT cidade FROM clientes GROUP BY cidade HAVING COUNT(*) > 1"),
    ("postgresql", "DISTINCT", "SELECT DISTINCT cidade FROM clientes"),
    ("postgresql", "SUM/AVG/MIN/MAX", "SELECT SUM(saldo) FROM clientes"),
    ("postgresql", "JOIN", "SELECT c.nome FROM clientes c JOIN clientes d ON c.id = d.id"),
    ("postgresql", "UNION", "SELECT nome FROM clientes UNION SELECT nome FROM clientes"),
    ("postgresql", "subconsulta", "SELECT * FROM clientes WHERE id IN (SELECT id FROM clientes)"),
    ("postgresql", "CTE (WITH)", "WITH x AS (SELECT * FROM clientes) SELECT * FROM x"),
    ("postgresql", "window function", "SELECT nome, ROW_NUMBER() OVER (ORDER BY nome) FROM clientes"),
    ("postgresql", "CASE", "SELECT CASE WHEN id = 1 THEN 'um' ELSE 'outro' END FROM clientes"),
    ("postgresql", "expressao", "SELECT saldo * 1.1 FROM clientes"),
    ("postgresql", "funcao escalar", "SELECT upper(nome) FROM clientes"),
    ("postgresql", "alias de tabela", "SELECT c.nome FROM clientes AS c"),
    ("postgresql", "WHERE sobre chave Sequence", "SELECT * FROM clientes WHERE id = 2"),
    ("postgresql", "INSERT ... RETURNING", "INSERT INTO clientes (nome) VALUES ('Zeca') RETURNING id"),
    ("postgresql", "ON CONFLICT (upsert)", "INSERT INTO clientes (id, nome) VALUES (1,'X') ON CONFLICT (id) DO UPDATE SET nome='X'"),
    ("postgresql", "SET TRANSACTION ISOLATION", "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"),
    ("postgresql", "SELECT sem FROM", "SELECT 1"),
    ("postgresql", "COPY", "COPY clientes FROM '/tmp/x.csv' CSV"),
    ("postgresql", "COMMENT ON", "COMMENT ON TABLE clientes IS 'cadastro'"),
    ("postgresql", "CREATE SEQUENCE", "CREATE SEQUENCE s1"),
    ("postgresql", "ANALYZE", "ANALYZE clientes"),
    ("postgresql", "VACUUM", "VACUUM clientes"),
    ("postgresql", "PREPARE", "PREPARE p1 AS SELECT * FROM clientes"),
    ("postgresql", "parametro $1", "SELECT * FROM clientes WHERE id = $1"),
    ("postgresql", "SELECT FOR UPDATE", "SELECT * FROM clientes WHERE id = 1 FOR UPDATE"),
    ("postgresql", "SET", "SET search_path TO public"),
    ("postgresql", "SHOW (variavel)", "SHOW server_version"),
    # ------------------------------------------------------------ MariaDB(R)
    ("mariadb", "REPLACE INTO", "REPLACE INTO clientes (id, nome) VALUES (1, 'X')"),
    ("mariadb", "INSERT ... ON DUPLICATE KEY", "INSERT INTO clientes (id,nome) VALUES (1,'X') ON DUPLICATE KEY UPDATE nome='X'"),
    ("mariadb", "SHOW TABLES", "SHOW TABLES"),
    ("mariadb", "SHOW DATABASES", "SHOW DATABASES"),
    ("mariadb", "SHOW CREATE TABLE", "SHOW CREATE TABLE clientes"),
    ("mariadb", "DESCRIBE", "DESCRIBE clientes"),
    ("mariadb", "USE", "USE loja"),
    ("mariadb", "LIMIT com virgula", "SELECT * FROM clientes LIMIT 0, 2"),
    ("mariadb", "CHECK constraint", "ALTER TABLE clientes ADD CONSTRAINT c1 CHECK (saldo >= 0)"),
    ("mariadb", "coluna gerada", "ALTER TABLE clientes ADD COLUMN dobro DECIMAL(12,2) AS (saldo*2) PERSISTENT"),
    ("mariadb", "EXCEPT", "SELECT nome FROM clientes EXCEPT SELECT nome FROM clientes"),
    ("mariadb", "INTERSECT", "SELECT nome FROM clientes INTERSECT SELECT nome FROM clientes"),
    ("mariadb", "CREATE EVENT", "CREATE EVENT e1 ON SCHEDULE EVERY 1 DAY DO SELECT 1"),
    ("mariadb", "CREATE ROLE", "CREATE ROLE gerente"),
    ("mariadb", "CREATE SEQUENCE", "CREATE SEQUENCE s1 START WITH 100"),
    ("mariadb", "AS OF (system versioned)", "SELECT * FROM clientes FOR SYSTEM_TIME AS OF NOW()"),
    ("mariadb", "MATCH ... AGAINST", "SELECT * FROM clientes WHERE MATCH(nome) AGAINST('Adriano')"),
    ("mariadb", "CREATE FUNCTION", "CREATE FUNCTION f1() RETURNS INT RETURN 1"),
    ("mariadb", "ANALYZE statement", "ANALYZE SELECT * FROM clientes"),
    ("mariadb", "SET autocommit", "SET autocommit = 0"),
    ("mariadb", "LOAD DATA INFILE", "LOAD DATA INFILE '/tmp/x.csv' INTO TABLE clientes"),
    ("mariadb", "backtick", "SELECT `nome` FROM `clientes`"),
    ("mariadb", "CREATE TRIGGER (controle)", "CREATE TRIGGER tg BEFORE INSERT ON clientes FOR EACH ROW BEGIN SET NEW.cidade = 'Blumenau'; END"),
    ("mariadb", "DROP TRIGGER", "DROP TRIGGER tg"),
    ("mariadb", "CREATE PROCEDURE (controle)", "CREATE PROCEDURE p1() BEGIN DECLARE x INT DEFAULT 1; SET x = x + 1; END"),
    ("mariadb", "CALL (controle)", "CALL p1()"),
    ("mariadb", "DROP PROCEDURE", "DROP PROCEDURE p1"),
    # -------------------------------------------------------------- MySQL(R)
    ("mysql", "JSON_EXTRACT", "SELECT JSON_EXTRACT(nome, '$.a') FROM clientes"),
    ("mysql", "coluna tipo JSON", "CREATE TABLE j (dados JSON)"),
    ("mysql", "window function", "SELECT nome, RANK() OVER (ORDER BY nome) FROM clientes"),
    ("mysql", "CTE recursiva", "WITH RECURSIVE r AS (SELECT 1 AS n) SELECT * FROM r"),
    ("mysql", "CREATE ROLE", "CREATE ROLE app_ro"),
    ("mysql", "SET ROLE", "SET ROLE app_ro"),
    ("mysql", "GRANT com role", "GRANT app_ro TO leitor"),
    ("mysql", "SELECT ... FOR SHARE", "SELECT * FROM clientes WHERE id = 1 FOR SHARE"),
    ("mysql", "LATERAL", "SELECT * FROM clientes, LATERAL (SELECT 1) x"),
    ("mysql", "VALUES statement", "VALUES ROW(1,2)"),
    ("mysql", "TABLE statement", "TABLE clientes"),
    ("mysql", "GROUP_CONCAT", "SELECT GROUP_CONCAT(nome) FROM clientes"),
    ("mysql", "INSERT multi-linha", "INSERT INTO clientes (nome) VALUES ('A'),('B')"),
    ("mysql", "OPTIMIZE TABLE", "OPTIMIZE TABLE clientes"),
    ("mysql", "CHECK TABLE", "CHECK TABLE clientes"),
    ("mysql", "SHOW VARIABLES", "SHOW VARIABLES LIKE 'version'"),
    ("mysql", "SHOW STATUS", "SHOW STATUS"),
    ("mysql", "SHOW PROCESSLIST", "SHOW PROCESSLIST"),
    ("mysql", "KILL", "KILL 1"),
    ("mysql", "information_schema", "SELECT * FROM information_schema.tables"),
    ("mysql", "FLUSH", "FLUSH TABLES"),
    ("mysql", "LOCK TABLES", "LOCK TABLES clientes WRITE"),
    ("mysql", "START TRANSACTION READ ONLY", "START TRANSACTION READ ONLY"),
    # ------------------------------------------------------------- SQLite(R)
    ("sqlite", "CREATE TABLE IF NOT EXISTS", "CREATE TABLE IF NOT EXISTS t2 (id INTEGER PRIMARY KEY)"),
    ("sqlite", "INSERT OR REPLACE", "INSERT OR REPLACE INTO clientes (id,nome) VALUES (1,'X')"),
    ("sqlite", "INSERT OR IGNORE", "INSERT OR IGNORE INTO clientes (id,nome) VALUES (1,'X')"),
    ("sqlite", "UPSERT", "INSERT INTO clientes (id,nome) VALUES (1,'X') ON CONFLICT(id) DO NOTHING"),
    ("sqlite", "PRAGMA", "PRAGMA table_info(clientes)"),
    ("sqlite", "ATTACH DATABASE", "ATTACH DATABASE '/tmp/o.db' AS o"),
    ("sqlite", "sqlite_master", "SELECT name FROM sqlite_master"),
    ("sqlite", "AUTOINCREMENT / last id", "SELECT last_insert_rowid()"),
    ("sqlite", "rowid", "SELECT rowid FROM clientes"),
    ("sqlite", "ORDER BY 2 clausulas", "SELECT * FROM clientes ORDER BY cidade, nome"),
    ("sqlite", "LIMIT sem ORDER", "SELECT * FROM clientes LIMIT 1 OFFSET 1"),
    ("sqlite", "VACUUM", "VACUUM"),
    ("sqlite", "ALTER TABLE RENAME", "ALTER TABLE clientes RENAME TO clientes2"),
    ("sqlite", "ALTER TABLE DROP COLUMN", "ALTER TABLE clientes DROP COLUMN cidade"),
    ("sqlite", "CREATE VIRTUAL TABLE (FTS5)", "CREATE VIRTUAL TABLE f USING fts5(corpo)"),
    ("sqlite", "REPLACE (funcao)", "SELECT replace(nome,'a','b') FROM clientes"),
    ("sqlite", "coalesce/ifnull", "SELECT ifnull(cidade,'?') FROM clientes"),
    ("sqlite", "datetime()", "SELECT datetime('now')"),
    ("sqlite", "SAVEPOINT (controle)", "SAVEPOINT sp1"),
    # ----------------------------------------------------------- Cassandra(R)
    ("cassandra", "CREATE KEYSPACE", "CREATE KEYSPACE ks WITH replication = {'class':'SimpleStrategy'}"),
    ("cassandra", "USE keyspace", "USE ks"),
    ("cassandra", "CREATE TABLE (PRIMARY KEY composta)", "CREATE TABLE t (pk text, ck text, v text, PRIMARY KEY ((pk), ck))"),
    ("cassandra", "INSERT ... USING TTL", "INSERT INTO clientes (id,nome) VALUES (9,'X') USING TTL 86400"),
    ("cassandra", "UPDATE ... USING TIMESTAMP", "UPDATE clientes USING TIMESTAMP 1 SET nome='X' WHERE id=1"),
    ("cassandra", "IF NOT EXISTS (LWT)", "INSERT INTO clientes (id,nome) VALUES (9,'X') IF NOT EXISTS"),
    ("cassandra", "CONSISTENCY", "CONSISTENCY QUORUM"),
    ("cassandra", "ALLOW FILTERING", "SELECT * FROM clientes WHERE cidade = 'Blumenau' ALLOW FILTERING"),
    ("cassandra", "token()", "SELECT * FROM clientes WHERE token(id) > 0"),
    ("cassandra", "BATCH", "BEGIN BATCH INSERT INTO clientes (id,nome) VALUES (9,'X') APPLY BATCH"),
    ("cassandra", "CREATE MATERIALIZED VIEW", "CREATE MATERIALIZED VIEW mv AS SELECT * FROM clientes WHERE id IS NOT NULL PRIMARY KEY (id)"),
    ("cassandra", "CREATE TYPE (UDT)", "CREATE TYPE endereco (rua text, cidade text)"),
    ("cassandra", "colecao (set/list/map)", "CREATE TABLE c2 (id int PRIMARY KEY, tags set<text>)"),
    ("cassandra", "counter", "UPDATE clientes SET saldo = saldo + 1 WHERE id = 1"),
    ("cassandra", "CREATE CUSTOM INDEX (SAI)", "CREATE CUSTOM INDEX ix ON clientes (cidade) USING 'StorageAttachedIndex'"),
    ("cassandra", "TRUNCATE", "TRUNCATE clientes"),
    ("cassandra", "DESCRIBE KEYSPACES", "DESCRIBE KEYSPACES"),
    ("cassandra", "ANN / busca vetorial", "SELECT * FROM clientes ORDER BY vetor ANN OF [1.0] LIMIT 5"),
]



# ------------------------------------------------- as EQUIVALENCIAS (categoria c)
#
# O gap so e gap quando NAO ha outro caminho. Este bloco prova o outro caminho:
# para cada comando SQL que o bloco de cima recusou, a operacao do protocolo
# que faz a mesma coisa — rodada de verdade, com a resposta colada.
#
# Sem ele a lista de gaps mentiria por omissao: «o PhxSql nao tem INSERT» e
# verdade sobre a LINGUAGEM e falso sobre o MOTOR.
EQUIVALENCIAS = [
    ("SELECT * FROM t", {"op": "varrer", "database": BANCO, "tabela": "clientes", "max": 2}),
    ("SELECT ... WHERE chave = ?", {"op": "buscar", "database": BANCO, "tabela": "itens",
                                    "indice": "porId", "chave": [1]}),
    ("SELECT ... WHERE col_sem_indice = ?", {"op": "varrer", "database": BANCO, "tabela": "clientes",
                                             "max": 100,
                                             "onde": [{"coluna": "cidade", "op": "=", "valor": "Blumenau"}]}),
    ("SELECT ... WHERE col LIKE '%x%'", {"op": "varrer", "database": BANCO, "tabela": "clientes",
                                         "max": 100,
                                         "onde": [{"coluna": "nome", "op": "contem", "valor": "dria"}]}),
    ("INSERT", {"op": "inserir", "database": BANCO, "tabela": "clientes",
                "valores": {"nome": "Zeca", "cidade": "Itajai", "saldo": "1.00"}}),
    ("INSERT multi-linha", {"op": "inserir_lote", "database": BANCO, "tabela": "clientes",
                            "linhas": [{"nome": "Dora"}, {"nome": "Elias"}]}),
    ("UPDATE ... WHERE pk", {"op": "atualizar", "database": BANCO, "tabela": "clientes", "rowid": 1,
                             "valores": {"nome": "Adriano", "cidade": "Gaspar", "saldo": "10.00"}}),
    ("DELETE (soft)", {"op": "excluir", "database": BANCO, "tabela": "clientes", "rowid": 2,
                       "motivo": "sonda"}),
    ("ROLLBACK do DELETE (nao ha em SQL padrao)", {"op": "restaurar", "database": BANCO,
                                                   "tabela": "clientes", "rowid": 2}),
    ("CREATE TABLE", {"op": "criar_tabela", "database": BANCO, "tabela": "fornecedores",
                      "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                                  {"nome": "nome", "tipo": "Str(40)"}],
                      "indices": [{"nome": "porId", "colunas": ["id"], "unico": True, "primario": True}]}),
    ("ALTER TABLE ADD COLUMN", {"op": "acrescentar_coluna", "database": BANCO, "tabela": "fornecedores",
                                "nome": "uf", "tipo": "Str(2)", "padrao": "SC"}),
    ("ALTER TABLE RENAME", {"op": "renomear_tabela", "database": BANCO, "tabela": "fornecedores",
                            "destino": "fornecedores2"}),
    ("DROP TABLE", {"op": "excluir_tabela", "database": BANCO, "tabela": "fornecedores2",
                    "confirmar": "fornecedores2"}),
    ("SHOW DATABASES", {"op": "bancos"}),
    ("SHOW TABLES", {"op": "tabelas", "database": BANCO}),
    ("DESCRIBE / PRAGMA table_info", {"op": "esquema", "database": BANCO, "tabela": "clientes"}),
    ("information_schema.tables", {"op": "sistabelas", "database": BANCO}),
    ("information_schema.columns", {"op": "siscolunas", "database": BANCO, "tabela": "clientes"}),
    ("JOIN", {"op": "juntar", "database": BANCO,
              "a": {"tabela": "clientes", "chave": "id"},
              "b": {"tabela": "itens", "chave": "id"}, "tipo": "interna"}),
    ("UNION ALL", {"op": "unir", "database": BANCO, "tabelas": ["clientes", "clientes"], "modo": "tudo"}),
    # As duas linhas de baixo usam o valor que o CATALOGO documenta -- e o motor
    # recusa as duas. Ficam aqui de proposito: e um achado, nao um erro da sonda.
    ("UNION (valor do catalogo: distinto)", {"op": "unir", "database": BANCO,
                                             "tabelas": ["clientes", "clientes"], "modo": "distinto"}),
    ("UNION (valor que o motor aceita: distinta)", {"op": "unir", "database": BANCO,
                                                    "tabelas": ["clientes", "clientes"],
                                                    "modo": "distinta"}),
    ("GROUP BY cruzado (exemplo do catalogo: somar)", {"op": "pivotar", "database": BANCO,
                                                       "tabela": "clientes", "chave": "cidade",
                                                       "valor": "saldo", "agregador": "somar"}),
    ("GROUP BY cruzado (a forma que o motor aceita)", {"op": "pivotar", "database": BANCO,
                                                       "tabela": "clientes",
                                                       "linhas": [{"campo": "cidade"}],
                                                       "valor": "saldo", "agregador": "soma"}),
    ("catalogo de pivotar (o que ele documenta)", {"op": "catalogo", "operacao": "pivotar"}),
    ("MATCH ... AGAINST (FTS)", {"op": "procurar_texto", "database": BANCO, "tabela": "chamados",
                                 "indice": "porCorpo", "palavra": "fenix"}),
    ("CREATE SEQUENCE / nextval", {"op": "sequencias", "database": BANCO}),
    ("ALTER SEQUENCE RESTART", {"op": "ajustar_sequencia", "database": BANCO, "tabela": "clientes",
                                "proxima": 5000}),
    ("COPY TO / SELECT INTO OUTFILE", {"op": "exportar", "database": BANCO, "tabela": "itens",
                                       "formato": "csv"}),
    ("LOAD DATA INFILE (conferencia)", {"op": "importar_conferir", "database": BANCO,
                                        "tabela": "itens", "formato": "csv",
                                        "texto": "id,descricao\n7,cha"}),
    ("CHECK TABLE", {"op": "verificar", "database": BANCO, "tabela": "clientes"}),
    ("OPTIMIZE / REINDEX", {"op": "reindexar", "database": BANCO, "tabela": "clientes"}),
    ("CHECKSUM TABLE", {"op": "checksum", "database": BANCO, "tabela": "itens"}),
    ("SHOW PROCESSLIST", {"op": "sessoes"}),
    ("SHOW GRANTS / pg_roles", {"op": "usuarios"}),
    ("BULKINSERT (LOCK TABLES ... WRITE)", {"op": "bulkinsert", "database": BANCO,
                                            "tabela": "itens", "ligado": True}),
    ("BULKINSERT off", {"op": "bulkinsert", "database": BANCO, "tabela": "itens", "ligado": False}),
    ("BEGIN com escopo declarado", {"op": "begin", "database": BANCO,
                                    "escopo": ["clientes"], "timeout_ms": 5000}),
    ("SAVEPOINT", {"op": "savepoint", "nome": "sp1"}),
    ("ROLLBACK TO SAVEPOINT", {"op": "rollback_para", "nome": "sp1"}),
    ("COMMIT", {"op": "commit"}),
    ("AS OF / historico da linha", {"op": "diario", "database": BANCO, "tabela": "clientes", "max": 5}),
    ("lixeira (nao ha equivalente em SQL)", {"op": "lixeira", "database": BANCO, "tabela": "clientes",
                                             "limite": 5}),
]


# ------------------------------------------------------------- os ACHADOS
#
# Quatro coisas que a sonda achou na primeira corrida e que nao eram o
# objetivo dela. Ficam medidas aqui para que a proxima rodada as reencontre
# — ou prove que sumiram.
ACHADOS = [
    ("o WHERE sobre chave Sequence recusa (o irmao Int8 passa)",
     {"op": "sql", "database": BANCO, "texto": "SELECT * FROM clientes WHERE id = 2"}),
    ("o mesmo WHERE sobre chave Int8 passa — o controle do de cima",
     {"op": "sql", "database": BANCO, "texto": "SELECT * FROM itens WHERE id = 1"}),
    ("FROM schema.tabela inexistente vaza erro cru do SO, com repetir:true",
     {"op": "sql", "database": BANCO, "texto": "SELECT * FROM filial.clientes"}),
    ("FROM tabela inexistente (sem schema) recusa direito, com repetir:false",
     {"op": "sql", "database": BANCO, "texto": "SELECT * FROM naoexiste"}),
    ("o varrer faz AND de duas condicoes — o substrato existe",
     {"op": "varrer", "database": BANCO, "tabela": "clientes", "max": 100,
      "onde": [{"coluna": "cidade", "op": "=", "valor": "Blumenau"},
               {"coluna": "nome", "op": "=", "valor": "Adriano"}]}),
    ("e o SELECT recusa a MESMA pergunta",
     {"op": "sql", "database": BANCO,
      "texto": "SELECT * FROM clientes WHERE cidade = 'Blumenau' AND nome = 'Adriano'"}),
    ("o .fts dobra acento: fênix acha fenix",
     {"op": "procurar_texto", "database": BANCO, "tabela": "chamados",
      "indice": "porCorpo", "palavra": "f\u00eanix"}),
    ("e nao faz prefixo: fen nao acha fenix",
     {"op": "procurar_texto", "database": BANCO, "tabela": "chamados",
      "indice": "porCorpo", "palavra": "fen"}),
]


# ------------------------------------------- a CONFERENCIA dos sprints antigos
#
# As respostas O, P e Q dizem que tal sprint de `docs/SPRINTS-MARIADB.md` ou de
# `docs/SPRINTS-CASSANDRA.md` ainda esta aberto. Essas tres operacoes sao a
# prova disso: o interruptor da exclusao na janela EXISTE (sprint fechado), o
# job NAO tem campo de replica e o source NAO guarda a posicao de cada replica
# (sprints abertos). Sem elas a conferencia seria leitura de codigo, e leitura
# de codigo diz o que deveria acontecer, nao o que acontece.
#
# Ela roda POR ULTIMO de proposito: o `CALL p6(1)` daqui grava uma linha, e
# rodando antes ela mudaria as contagens que o bloco das equivalencias publica
# -- o que ja aconteceu uma vez e so apareceu quando as citacoes das respostas
# pararam de bater com o `resultados.json`.
CONFERENCIA = [
    ("config: os interruptores de durabilidade e o bloco de papeis", {"op": "config"}),
    ("jobs: ha campo que desligue o job na replica?", {"op": "jobs"}),
    ("replicacao_estado: o source guarda a posicao de cada replica?", {"op": "replicacao_estado"}),
    # O «degrau seguinte do interpretador» (sprint 12 do MariaDB) e a lista do
    # que o corpo de rotina ainda recusa. Duas passam, quatro recusam — e as
    # quatro recusas nomeiam o motivo, que e o que faz este item ser MEDICAO e
    # nao sprint: so o Profiler diz em qual delas alguem esbarra de verdade.
    ("corpo de rotina: WHILE (passa)", {"op": "sql", "database": BANCO, "texto":
     "CREATE PROCEDURE p3() BEGIN DECLARE x INT DEFAULT 0; WHILE x < 3 DO SET x = x + 1; END WHILE; END"}),
    ("corpo de rotina: INSERT (passa)", {"op": "sql", "database": BANCO, "texto":
     "CREATE PROCEDURE p6(IN v INT) BEGIN INSERT INTO clientes (nome) VALUES ('via proc'); END"}),
    ("corpo de rotina: CALL do que passou", {"op": "sql", "database": BANCO, "texto": "CALL p6(1)"}),
    ("corpo de rotina: UPDATE (recusa)", {"op": "sql", "database": BANCO, "texto":
     "CREATE PROCEDURE p2() BEGIN UPDATE clientes SET nome='X' WHERE id=1; END"}),
    ("corpo de rotina: CASE (recusa)", {"op": "sql", "database": BANCO, "texto":
     "CREATE PROCEDURE p4() BEGIN DECLARE x INT; CASE x WHEN 1 THEN SET x=2; END CASE; END"}),
    ("corpo de rotina: CURSOR (recusa)", {"op": "sql", "database": BANCO, "texto":
     "CREATE PROCEDURE p5() BEGIN DECLARE c CURSOR FOR SELECT id FROM clientes; END"}),
    ("corpo de rotina: transacao (recusa)", {"op": "sql", "database": BANCO, "texto":
     "CREATE PROCEDURE p7() BEGIN START TRANSACTION; COMMIT; END"}),
]


def corta(t, n=220):
    t = " ".join(str(t).split())
    return t if len(t) <= n else t[: n - 1] + "…"


def main():
    filtro = sys.argv[1] if len(sys.argv) > 1 else ""
    saida = {"porta": PORTA, "controle": [], "resultados": []}
    with Servidor() as sv:
        preparar(sv)

        print("== CONTROLE POSITIVO (tem de passar inteiro) ==")
        falhou = 0
        for s in CONTROLE:
            r = sv.sql(s)
            ok = bool(r.get("ok"))
            falhou += 0 if ok else 1
            print(f"  [{'OK  ' if ok else 'FALHA'}] {s}")
            if not ok:
                print(f"         {corta(r.get('erro'))}")
            saida["controle"].append({"sql": s, "ok": ok, "erro": r.get("erro", "")})
        if falhou:
            raise SystemExit(
                f"\nO CONTROLE FALHOU em {falhou} de {len(CONTROLE)}. "
                "Zero nos outros blocos nao vale — conserte a sonda antes."
            )

        motor_atual = None
        contagem = {}
        for motor, rotulo, s in CANDIDATOS:
            if filtro and motor != filtro:
                continue
            if motor != motor_atual:
                motor_atual = motor
                print(f"\n== {motor.upper()} ==")
            r = sv.sql(s)
            ok = bool(r.get("ok"))
            c = contagem.setdefault(motor, [0, 0])
            c[0 if ok else 1] += 1
            marca = "ACEITO  " if ok else "RECUSADO"
            print(f"  [{marca}] {rotulo}")
            print(f"             sql: {s}")
            print(f"             {'resp' if ok else 'erro'}: {corta(r.get('erro') or r)}")
            saida["resultados"].append(
                {
                    "motor": motor,
                    "rotulo": rotulo,
                    "sql": s,
                    "aceito": ok,
                    # Aceito guarda a RESPOSTA, e nao so o erro: uma lista de gap
                    # tambem se confere pelo que passou, e a resposta do que
                    # passou tem de estar no arquivo para poder ser citada.
                    "mensagem": corta(r.get("erro") or r.get("resultado") or r, 400),
                }
            )

        print("\n== ACHADOS (o que a sonda topou sem procurar) ==")
        for rotulo, pedido in ACHADOS:
            r = sv.pedir(**pedido)
            ok = bool(r.get("ok"))
            print(f"  [{'OK  ' if ok else 'ERRO'}] {rotulo}")
            print(f"             {corta(r.get('erro') or r.get('resultado') or r)}")
            saida.setdefault("achados", []).append(
                {"o_que": rotulo, "pedido": pedido, "ok": ok,
                 "resposta": corta(r.get("erro") or r.get("resultado") or r, 400)}
            )

        print("\n== EQUIVALENCIAS PELO PROTOCOLO (o que existe com outra forma) ==")
        for rotulo, pedido in EQUIVALENCIAS:
            r = sv.pedir(**pedido)
            ok = bool(r.get("ok"))
            print(f"  [{'OK  ' if ok else 'ERRO'}] {rotulo}  ->  op {pedido['op']}")
            print(f"             {corta(r.get('erro') or r.get('resultado') or r)}")
            saida.setdefault("equivalencias", []).append(
                {"sql": rotulo, "op": pedido["op"], "ok": ok,
                 "resposta": corta(r.get("erro") or r.get("resultado") or r, 400)}
            )

        print("\n== CONFERENCIA DOS SPRINTS ANTIGOS ==")
        for rotulo, pedido in CONFERENCIA:
            r = sv.pedir(**pedido)
            ok = bool(r.get("ok"))
            print(f"  [{'OK  ' if ok else 'ERRO'}] {rotulo}")
            print(f"             {corta(r.get('erro') or r.get('resultado') or r, 400)}")
            saida.setdefault("conferencia", []).append(
                {"o_que": rotulo, "op": pedido["op"], "ok": ok,
                 "resposta": corta(r.get("erro") or r.get("resultado") or r, 900)}
            )

        print("\n== RESUMO ==")
        for motor, (a, rec) in contagem.items():
            print(f"  {motor:12s} aceitos {a:3d}   recusados {rec:3d}")

    destino = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resultados.json")
    with open(destino, "w") as f:
        json.dump(saida, f, ensure_ascii=False, indent=1)
    print(f"\ngravado: {destino}")


if __name__ == "__main__":
    main()
