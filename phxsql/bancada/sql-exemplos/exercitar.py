#!/usr/bin/env python3
"""Exercita, contra um phxsqld DE PE, os cinco pedidos do PDF das 26 perguntas
que cabem a esta frente (A, E, F, G, H).

    target/release/phxsqld ja tem de estar compilado (nao compila aqui)
    python3 bancada/sql-exemplos/exercitar.py

Le o codigo diz o que DEVERIA acontecer; rodar contra o motor diz o que
ACONTECE. Este script nao redige nada -- ele imprime a resposta CRUA do
protocolo (o JSON que vem em "resultado"), para que `docs/pdf/respostas/*.md`
cole exatamente o que saiu, com data e commit.

O que cada secao prova:

  A) toda forma de SQL que `docs/SQL.md` e `docs/TRIGGERS.md` dizem aceitar
     (ou dizem recusar, com o motivo) e mandada de verdade -- ok/erro fica
     registrado, e nao suposto. Comando que o doc lista e o motor recusa e
     um achado; comando que o motor aceita e o doc nao lista, tambem.
  E) uma stored procedure com IN/OUT, DECLARE, WHILE e SELECT...INTO.
  F) um trigger BEFORE que normaliza e recusa por SIGNAL, e um AFTER que
     audita -- os dois exercitados com INSERT de verdade.
  G) CREATE DATABASE (via op nativa), CREATE TABLE com colunas de varios
     tipos, ALTER TABLE ADD COLUMN, e a chave estrangeira nascendo
     restringir/cascata/conferida -- com o `esquema` devolvido de prova.
  H) systables, syscolumns e catalogo, com a saida real do motor.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from datetime import datetime, timezone

PORTA = int(os.environ.get("PHX_F1_PORTA", "6100"))
BASE = f"/tmp/phx-f1-{os.getpid()}"
BINARIO = "target/release/phxsqld"
TOKEN = "t"


class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer."""

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(f"{BINARIO} nao existe -- o binario ja tem de estar compilado")
        shutil.rmtree(BASE, ignore_errors=True)
        os.makedirs(BASE + "/dados", exist_ok=True)
        cfg = BASE + "/config.json"
        with open(cfg, "w") as f:
            json.dump(
                {
                    "bind": f"127.0.0.1:{PORTA}",
                    "base": BASE + "/dados",
                    "token": TOKEN,
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
        self.s = socket.create_connection(("127.0.0.1", PORTA), 5)
        self.f = self.s.makefile("rwb")
        return self

    def __exit__(self, *_):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        shutil.rmtree(BASE, ignore_errors=True)

    def pedir(self, **kw):
        kw.setdefault("token", TOKEN)
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        # A resposta util vem em `resultado`; o nivel de cima so tem `ok`/`op`.
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r

    def sql(self, texto, database="loja"):
        return self.pedir(op="sql", database=database, texto=texto)


def cab(titulo):
    print("\n" + "#" * 78)
    print("# " + titulo)
    print("#" * 78 + "\n")


def sub(titulo):
    print("\n--- " + titulo + " ---\n")


def js(obj):
    return json.dumps(obj, ensure_ascii=False, sort_keys=False)


# ======================================================================
# A) lista de comandos SQL funcionais no phxsql
# ======================================================================


def item_a(sv):
    cab("A) lista de comandos SQL funcionais no phxsql")

    sv.pedir(op="criar_database", database="loja")
    sv.pedir(
        op="criar_tabela",
        database="loja",
        tabela="clientes",
        colunas=[
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "nome", "tipo": "Str(40)"},
            {"nome": "cidade", "tipo": "Str(40)"},
        ],
        indices=[{"nome": "porId", "colunas": ["id"], "unico": True, "primario": True}],
    )
    for i, nome, cidade in [
        (1, "Ana", "Blumenau"),
        (2, "Bruno", "Joinville"),
        (3, "Carla", None),
        (4, "Duda", "Blumenau"),
        (5, "Elis", "Itajai"),
    ]:
        linha = {"id": i, "nome": nome}
        if cidade is not None:
            linha["cidade"] = cidade
        sv.pedir(op="inserir", database="loja", tabela="clientes", linha=linha)

    # (rotulo, sql, esperado) -- esperado e so para o script anotar a favor
    # de quem le; o "ok"/"erro" real sai do motor, nao do esperado.
    comandos = [
        # --- SELECT, aceito pelo tradutor -----------------------------
        ("SELECT * simples", "SELECT * FROM clientes", "ok"),
        ("SELECT com colunas e apelido", "SELECT id, nome AS quem FROM clientes", "ok"),
        ("SELECT com AS no proprio COUNT", "SELECT COUNT(*) AS quantos FROM clientes", "ok"),
        ("COUNT(*)", "SELECT COUNT(*) FROM clientes", "ok"),
        ("FROM banco.tabela (3 partes: so 2 aqui)", "SELECT * FROM loja.clientes", "ok"),
        ("WHERE = na coluna indexada", "SELECT * FROM clientes WHERE id = 1", "ok"),
        ("WHERE <>", "SELECT * FROM clientes WHERE id <> 1", "ok"),
        ("WHERE <", "SELECT * FROM clientes WHERE id < 3", "ok"),
        ("WHERE <=", "SELECT * FROM clientes WHERE id <= 3", "ok"),
        ("WHERE >", "SELECT * FROM clientes WHERE id > 1", "ok"),
        ("WHERE >=", "SELECT * FROM clientes WHERE id >= 1", "ok"),
        ("ORDER BY asc", "SELECT * FROM clientes ORDER BY id", "ok"),
        ("ORDER BY desc", "SELECT * FROM clientes ORDER BY id DESC", "ok"),
        ("LIMIT", "SELECT * FROM clientes LIMIT 2", "ok"),
        ("LIMIT + OFFSET", "SELECT * FROM clientes LIMIT 2 OFFSET 1", "ok"),
        # --- SELECT, recusado por falta de substrato (documentado) -----
        ("WHERE em coluna SEM indice", "SELECT * FROM clientes WHERE cidade = 'Blumenau'", "erro"),
        ("ORDER BY em coluna SEM indice", "SELECT * FROM clientes ORDER BY cidade", "erro"),
        ("DISTINCT", "SELECT DISTINCT nome FROM clientes", "erro"),
        ("AND", "SELECT * FROM clientes WHERE id = 1 AND nome = 'Ana'", "erro"),
        ("LIKE", "SELECT * FROM clientes WHERE nome LIKE 'A%'", "erro"),
        ("IN", "SELECT * FROM clientes WHERE id IN (1, 2)", "erro"),
        ("BETWEEN", "SELECT * FROM clientes WHERE id BETWEEN 1 AND 3", "erro"),
        ("IS NULL", "SELECT * FROM clientes WHERE cidade IS NULL", "erro"),
        ("GROUP BY", "SELECT nome, COUNT(*) FROM clientes GROUP BY nome", "erro"),
        ("JOIN", "SELECT * FROM clientes JOIN pedidos ON clientes.id = pedidos.cliente_id", "erro"),
        ("SUM (agregado que nao e COUNT)", "SELECT SUM(id) FROM clientes", "erro"),
        # --- verbos ainda nao ligados nesta camada -----------------------
        ("INSERT via SQL", "INSERT INTO clientes (id, nome) VALUES (9, 'Zeca')", "erro"),
        ("UPDATE via SQL", "UPDATE clientes SET nome = 'X' WHERE id = 1", "erro"),
        ("DELETE via SQL", "DELETE FROM clientes WHERE id = 1", "erro"),
        # --- DDL: so existe pela op nativa, e a recusa tem de dizer isso --
        ("CREATE TABLE via SQL", "CREATE TABLE x (id INT)", "erro"),
        ("CREATE DATABASE via SQL", "CREATE DATABASE outra", "erro"),
        ("ALTER TABLE via SQL", "ALTER TABLE clientes ADD COLUMN x INT", "erro"),
        # --- BULKINSERT: palavra reservada, comando de sessao -----------
        ("BULKINSERT(true)", "BULKINSERT(true)", "erro"),
        # --- transacao (op_sql detecta ANTES do tradutor de SELECT) -----
        ("BEGIN", "BEGIN", "ok"),
        ("COMMIT", "COMMIT", "ok"),
        ("BEGIN TRANSACTION", "BEGIN TRANSACTION", "ok"),
        ("COMMIT WORK", "COMMIT WORK", "ok"),
        ("ROLLBACK (sem transacao aberta)", "ROLLBACK", "ok-ou-erro"),
        ("START TRANSACTION", "START TRANSACTION", "ok"),
        ("SAVEPOINT", "SAVEPOINT antes_do_lote", "ok"),
        ("ROLLBACK TO SAVEPOINT", "ROLLBACK TO SAVEPOINT antes_do_lote", "ok-ou-erro"),
        ("RELEASE SAVEPOINT", "RELEASE SAVEPOINT antes_do_lote", "ok-ou-erro"),
        # SAVEPOINT/ROLLBACK TO/RELEASE nao fecham a transacao -- sem este
        # COMMIT ela ficaria aberta na CONEXAO e bloquearia todo DDL dos
        # itens seguintes (criar_database, criar_tabela, declarar_fk...),
        # que a mensagem do proprio motor diz que "nao entra em transacao".
        ("COMMIT (fecha a transacao desta bateria)", "COMMIT", "ok"),
        (
            "BEGIN TRANSACTION com clausulas completas",
            "BEGIN TRANSACTION SCOPE (clientes) SCOPE MODE STRICT TIMEOUT 5s "
            "LOCK TIMEOUT 500ms STATEMENT TIMEOUT 2s LOCK MODE AUTO",
            "ok",
        ),
        ("COMMIT (fecha de novo, conexao limpa para o resto do roteiro)", "COMMIT", "ok"),
        ("COMMIT AND CHAIN (nao existe)", "COMMIT AND CHAIN", "erro"),
        (
            "SET TRANSACTION ISOLATION LEVEL (fora do vocabulario do detector)",
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
            "erro",
        ),
        # --- gatilho e procedimento entram pela MESMA op --------------
        (
            "CREATE TRIGGER",
            "CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW "
            "SET NEW.cidade = UPPER(TRIM(NEW.cidade))",
            "ok",
        ),
        ("SHOW TRIGGERS", "SHOW TRIGGERS", "ok"),
        ("DROP TRIGGER", "DROP TRIGGER normaliza", "ok"),
        ("DROP TRIGGER IF EXISTS (ja excluido)", "DROP TRIGGER IF EXISTS normaliza", "ok"),
        (
            "CREATE PROCEDURE",
            "CREATE PROCEDURE dobro(IN x INT, OUT y INT) SET y = x * 2",
            "ok",
        ),
        ("CALL", "CALL dobro(21)", "ok"),
        ("SHOW PROCEDURES", "SHOW PROCEDURES", "ok"),
        ("SHOW PROCEDURE STATUS", "SHOW PROCEDURE STATUS", "ok"),
        ("DROP PROCEDURE", "DROP PROCEDURE dobro", "ok"),
    ]

    linhas_de_resultado = []
    for rotulo, texto, esperado in comandos:
        r = sv.sql(texto)
        ok = bool(r.get("ok"))
        marca = "OK  " if ok else "ERRO"
        print(f"[{marca}] {rotulo}")
        print(f"       SQL: {texto}")
        if ok:
            mostrar = {
                k: v
                for k, v in r.items()
                if k not in ("token",)
            }
            print(f"       -> {js(mostrar)[:300]}")
        else:
            print(f"       -> erro: {r.get('erro')}")
        print()
        linhas_de_resultado.append((rotulo, texto, ok, r.get("erro")))

    passou = sum(1 for _, _, ok, _ in linhas_de_resultado if ok)
    falhou = len(linhas_de_resultado) - passou
    print(f"RESUMO A: {len(linhas_de_resultado)} comandos, {passou} ok, {falhou} erro")
    return linhas_de_resultado


# ======================================================================
# E) exemplo de stored procedure em phxsql
# ======================================================================


def item_e(sv):
    cab("E) exemplo de stored procedure em phxsql")

    sub("criar a procedure: soma de 1 ate N, com IN/OUT, DECLARE e WHILE")
    r = sv.sql(
        "CREATE PROCEDURE somar_ate(IN ate INT, OUT total INT) BEGIN "
        "  DECLARE i INT DEFAULT 1; "
        "  SET total = 0; "
        "  WHILE i <= ate DO "
        "    SET total = total + i; "
        "    SET i = i + 1; "
        "  END WHILE; "
        "END"
    )
    print(js(r))

    sub("SHOW PROCEDURES -- o corpo guardado, verbatim")
    r = sv.sql("SHOW PROCEDURES")
    print(js(r))

    sub("CALL somar_ate(100) -- devolve o OUT em 'saida'")
    r = sv.sql("CALL somar_ate(100)")
    print(js(r))

    sub("CALL somar_ate(10) -- outro argumento, outro resultado")
    r = sv.sql("CALL somar_ate(10)")
    print(js(r))

    sub("segunda procedure: le o motor com SELECT ... INTO")
    r = sv.sql(
        "CREATE PROCEDURE resumo_clientes(OUT quantos INT, OUT primeiro_nome VARCHAR(40)) "
        "BEGIN "
        "  SELECT COUNT(*) INTO quantos FROM clientes; "
        "  SELECT nome INTO primeiro_nome FROM clientes WHERE id = 1; "
        "END"
    )
    print(js(r))
    r = sv.sql("CALL resumo_clientes()")
    print(js(r))

    sub("DROP PROCEDURE")
    print(js(sv.sql("DROP PROCEDURE somar_ate")))
    print(js(sv.sql("DROP PROCEDURE resumo_clientes")))


# ======================================================================
# F) exemplo de trigger em phxsql
# ======================================================================


def item_f(sv):
    cab("F) exemplo de trigger em phxsql")

    sv.pedir(
        op="criar_tabela",
        database="loja",
        tabela="auditoria",
        colunas=[
            {"nome": "evento", "tipo": "Str(120)"},
        ],
    )

    sub("BEFORE INSERT normaliza a cidade (maiuscula, sem espaco nas pontas)")
    print(js(sv.sql(
        "CREATE TRIGGER normaliza_cidade BEFORE INSERT ON clientes FOR EACH ROW "
        "SET NEW.cidade = UPPER(TRIM(NEW.cidade))"
    )))
    r = sv.pedir(
        op="inserir",
        database="loja",
        tabela="clientes",
        linha={"id": 100, "nome": "Fabio", "cidade": "  gaspar "},
    )
    print("inserir com cidade '  gaspar ':", js(r))
    r = sv.pedir(op="ler", database="loja", tabela="clientes", rowid=r.get("rowid"))
    print("linha gravada:", js(r))

    sub("BEFORE INSERT recusa por SIGNAL quando o nome vem vazio")
    print(js(sv.sql(
        "CREATE TRIGGER exige_nome BEFORE INSERT ON clientes FOR EACH ROW "
        "IF NEW.nome IS NULL OR NEW.nome = '' THEN "
        "  SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cliente sem nome nao entra'; "
        "END IF"
    )))
    r = sv.pedir(
        op="inserir",
        database="loja",
        tabela="clientes",
        linha={"id": 101, "nome": "", "cidade": "x"},
    )
    print("inserir com nome vazio (esperado: recusa):", js(r))

    sub("AFTER INSERT audita em outra tabela, vendo a linha como ela FICOU")
    print(js(sv.sql(
        "CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW "
        "INSERT INTO auditoria (evento) "
        "VALUES (CONCAT('entrou ', NEW.nome, ' de ', NEW.cidade))"
    )))
    r = sv.pedir(
        op="inserir",
        database="loja",
        tabela="clientes",
        linha={"id": 102, "nome": "Gilda", "cidade": " navegantes"},
    )
    print("inserir Gilda:", js(r))
    r = sv.pedir(op="varrer", database="loja", tabela="auditoria", max=10)
    print("tabela auditoria depois:", js(r))

    sub("SHOW TRIGGERS -- os tres, com o corpo guardado")
    print(js(sv.sql("SHOW TRIGGERS")))

    sub("DROP TRIGGER dos tres")
    for nome in ("normaliza_cidade", "exige_nome", "audita"):
        print(nome, "->", js(sv.sql(f"DROP TRIGGER {nome}")))


# ======================================================================
# G) exemplo de create database, table, column e ER
# ======================================================================


def item_g(sv):
    cab("G) exemplo de create database, table, column e ER")

    sub("CREATE DATABASE (op nativa criar_database -- SQL text nao tem DDL)")
    r = sv.pedir(op="criar_database", database="filial")
    print(js(r))

    sub("CREATE TABLE com colunas de varios tipos")
    r = sv.pedir(
        op="criar_tabela",
        database="filial",
        tabela="produtos",
        colunas=[
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "nome", "tipo": "Str(80)", "obrigatoria": True},
            {"nome": "preco", "tipo": "Decimal(15,2)"},
            {"nome": "criado_em", "tipo": "Date"},
            {"nome": "atualizado_em", "tipo": "DateTime"},
            {"nome": "ativo", "tipo": "Bool"},
            {"nome": "ficha", "tipo": "Memo"},
            {"nome": "uuid", "tipo": "Uuid"},
        ],
        indices=[{"nome": "porId", "colunas": ["id"], "unico": True, "primario": True}],
    )
    print(js(r))

    r = sv.pedir(
        op="criar_tabela",
        database="filial",
        tabela="pedidos",
        colunas=[
            {"nome": "id", "tipo": "Int8", "obrigatoria": True},
            {"nome": "produto_id", "tipo": "Int8"},
            {"nome": "quantidade", "tipo": "Int4"},
        ],
        indices=[
            {"nome": "porId", "colunas": ["id"], "unico": True, "primario": True},
            {"nome": "porProduto", "colunas": ["produto_id"]},
        ],
    )
    print("criar_tabela pedidos:", js(r))

    sub("ALTER TABLE ADD COLUMN -- op nativa acrescentar_coluna")
    r = sv.pedir(op="inserir", database="filial", tabela="produtos",
                 linha={"id": 1, "nome": "Parafuso", "preco": "1.50"})
    print("uma linha ja gravada antes do ALTER:", js(r))
    r = sv.pedir(
        op="acrescentar_coluna",
        database="filial",
        tabela="produtos",
        coluna={"nome": "categoria", "tipo": "Str(30)"},
    )
    print("acrescentar_coluna categoria:", js(r))

    sub("declarar chave estrangeira pedidos.produto_id -> produtos.id")
    sub("(1) sem dizer ao_excluir/ao_alterar -- nasce restringir/cascata/conferida")
    r = sv.pedir(
        op="declarar_fk",
        database="filial",
        tabela="pedidos",
        nome="fk_produto",
        colunas=["produto_id"],
        tabela_ref="produtos",
        colunas_ref=["id"],
    )
    print(js(r))

    sub("(2) a regra primordial: ao_excluir so aceita restringir -- cascata e recusada")
    r = sv.pedir(
        op="declarar_fk",
        database="filial",
        tabela="pedidos",
        nome="fk_produto_errada",
        colunas=["produto_id"],
        tabela_ref="produtos",
        colunas_ref=["id"],
        ao_excluir="cascata",
    )
    print(js(r))

    sub("(3) ao_alterar aceito explicitamente como cascata (o padrao, dito por extenso)")
    r = sv.pedir(op="excluir_fk", database="filial", tabela="pedidos", nome="fk_produto")
    print("excluir_fk para redeclarar:", js(r))
    r = sv.pedir(
        op="declarar_fk",
        database="filial",
        tabela="pedidos",
        nome="fk_produto",
        colunas=["produto_id"],
        tabela_ref="produtos",
        colunas_ref=["id"],
        ao_alterar="cascata",
    )
    print(js(r))

    sub("(4) verificar:false -- escolha ESCRITA de declarar sem conferir")
    r = sv.pedir(op="excluir_fk", database="filial", tabela="pedidos", nome="fk_produto")
    print("excluir_fk para redeclarar:", js(r))
    r = sv.pedir(
        op="declarar_fk",
        database="filial",
        tabela="pedidos",
        nome="fk_produto",
        colunas=["produto_id"],
        tabela_ref="produtos",
        colunas_ref=["id"],
        verificar=False,
    )
    print(js(r))
    r = sv.pedir(op="excluir_fk", database="filial", tabela="pedidos", nome="fk_produto")
    print("excluir_fk para deixar a chave definitiva:", js(r))
    r = sv.pedir(
        op="declarar_fk",
        database="filial",
        tabela="pedidos",
        nome="fk_produto",
        colunas=["produto_id"],
        tabela_ref="produtos",
        colunas_ref=["id"],
    )
    print("chave definitiva (a que fica para o restante da prova):", js(r))

    sub("a chave NASCE conferida: inserir filha apontando para pai que nao existe recusa")
    r = sv.pedir(
        op="inserir",
        database="filial",
        tabela="pedidos",
        linha={"id": 1, "produto_id": 999, "quantidade": 1},
    )
    print("pedido para produto_id=999 (nao existe):", js(r))
    r = sv.pedir(
        op="inserir",
        database="filial",
        tabela="pedidos",
        linha={"id": 1, "produto_id": 1, "quantidade": 3},
    )
    print("pedido para produto_id=1 (existe):", js(r))

    sub("regra primordial na GRAVACAO: excluir o pai com filha viva e recusado")
    r = sv.pedir(op="excluir", database="filial", tabela="produtos", rowid=1, motivo="teste")
    print("excluir produto 1, que tem pedido filho (esperado: recusa):", js(r))

    sub("o `esquema` devolvido -- o ER em forma de dado: colunas, indices e chaves")
    r = sv.pedir(op="esquema", database="filial", tabela="pedidos")
    print(js(r))
    r = sv.pedir(op="esquema", database="filial", tabela="produtos")
    print("esquema de produtos (a tabela mae):", js(r))


# ======================================================================
# H) exemplo de uso systables e syscolumns
# ======================================================================


def item_h(sv):
    cab("H) exemplo de uso systables e syscolumns")

    sub("systables -- uma linha por tabela do database 'loja'")
    r = sv.pedir(op="systables", database="loja")
    print(js(r))

    sub("sistabelas -- o sinonimo em portugues, mesmo database")
    r = sv.pedir(op="sistabelas", database="loja")
    print(js(r))

    sub("syscolumns -- todas as colunas de todas as tabelas do database")
    r = sv.pedir(op="syscolumns", database="loja")
    print(js(r))

    sub("syscolumns filtrado por tabela")
    r = sv.pedir(op="syscolumns", database="loja", tabela="clientes")
    print(js(r))

    sub("catalogo -- o inventario de operacoes que esta sessao pode chamar")
    r = sv.pedir(op="catalogo")
    total = r.get("total")
    print(f"total de operacoes visiveis: {total}")
    print(js({k: v for k, v in r.items() if k != "operacoes"})[:1500])

    sub("catalogo com 'operacao' -- o detalhe de uma operacao pedida por nome")
    r = sv.pedir(op="catalogo", operacao="declarar_fk")
    print(js(r))

    sub("catalogo com operacao inexistente -- diz que nao existe, e nao um 404 seco")
    r = sv.pedir(op="catalogo", operacao="xyzzy_nao_existe")
    print(js(r))


def main():
    print(f"corrida em {datetime.now(timezone.utc).isoformat()}")
    with Servidor() as sv:
        a = item_a(sv)
        item_e(sv)
        item_f(sv)
        item_g(sv)
        item_h(sv)
    print("\nfim.")
    return a


if __name__ == "__main__":
    sys.exit(0 if main() is not None else 1)
