#!/usr/bin/env python3
"""O DbLink contra um MariaDB DE VERDADE, num terceiro conteiner.

    python3 bancada/docker/montar-dois.py       # antes: phx-a e phx-b
    python3 bancada/docker/dblink-mariadb.py    # sobe o erp-mariadb e prova

Por que este roteiro existe separado do `exercitar-dois.py`: o pedido era
«dblink entre os dois PhxSql», e isso NAO EXISTE -- o DbLink fala `mysql`/
`mariadb` e `postgres`, e mais nada. Entre dois PhxSql o canal e a replicacao.
Entao a prova honesta do DbLink e contra o motor que ele realmente alcanca.

A senha do MariaDB entra por VARIAVEL DE AMBIENTE (`senha_env`), e nao no
config: e o caminho que o proprio DbLink oferece, e a ficha da ligacao devolve
`senha: "(oculta)"`. Senha em texto puro nao entra em arquivo nenhum aqui.
"""
import json, socket, subprocess, sys, time
from pathlib import Path

TOKEN = "token-do-teste-dois-docker"
SENHA_PHX = "segredo-do-teste"
SENHA_ERP = "leitor-do-teste"
TRAB = Path(__file__).resolve().parent

def sh(*a):
    return subprocess.run(a, capture_output=True, text=True)

class Cliente:
    def __init__(self, porta, prazo=120):
        # PRAZO FOLGADO, e de proposito: a primeira versao usava 8 s, o
        # `dblink_testar` contra uma porta que nao fala MySQL desiste em 10, e
        # eu quase relatei «o DbLink trava». Instrumento mais curto que a coisa
        # medida mede a si mesmo.
        self.s = socket.create_connection(("127.0.0.1", porta), 8)
        self.s.settimeout(prazo)
        self.f = self.s.makefile("rwb")
    def call(self, o):
        o.setdefault("token", TOKEN)
        self.f.write((json.dumps(o) + "\n").encode()); self.f.flush()
        return json.loads(self.f.readline().decode())

def subir_mariadb():
    sh("docker", "rm", "-f", "erp-mariadb")
    r = sh("docker", "run", "-d", "--name", "erp-mariadb", "--network", "phxnet",
           "--hostname", "erp-mariadb",
           "-e", "MARIADB_ROOT_PASSWORD=raiz-do-teste", "-e", "MARIADB_DATABASE=erp",
           "-e", "MARIADB_USER=leitor", "-e", f"MARIADB_PASSWORD={SENHA_ERP}",
           "mariadb:11")
    if r.returncode != 0:
        sys.exit(f"mariadb nao subiu: {r.stderr}")
    for i in range(1, 41):
        if sh("docker", "exec", "erp-mariadb", "mariadb", "-uleitor",
              f"-p{SENHA_ERP}", "-e", "SELECT 1", "erp").returncode == 0:
            print(f"· erp-mariadb pronto na tentativa {i}")
            return
        time.sleep(3)
    sys.exit("o mariadb nao ficou pronto")

def semear():
    sh("docker", "exec", "erp-mariadb", "mariadb", "-uroot", "-praiz-do-teste", "erp",
       "-e", """
       DROP TABLE IF EXISTS notas; DROP TABLE IF EXISTS fornecedores;
       CREATE TABLE fornecedores (id INT PRIMARY KEY, razao VARCHAR(60), uf CHAR(2), limite DECIMAL(12,2));
       INSERT INTO fornecedores VALUES (1,'Metalurgica Sul','RS',48000.00),
              (2,'Papelaria Norte','AM',7500.50),(3,'Tecidos Centro','GO',21300.00);
       CREATE TABLE notas (id INT PRIMARY KEY, fornecedor_id INT, valor DECIMAL(12,2));
       INSERT INTO notas VALUES (10,1,1200.00),(11,1,890.75),(12,3,4300.00);""")

def principal():
    subir_mariadb(); semear()
    # O phx-a precisa da variavel no ambiente DELE. Recriar nao perde nada: os
    # dados moram no volume.
    sh("docker", "rm", "-f", "phx-a")
    sh("docker", "run", "-d", "--name", "phx-a", "--network", "phxnet",
       "--hostname", "phx-a", "-e", f"ERP_SENHA={SENHA_ERP}",
       "-p", "6500:5000", "-p", "6501:5001",
       "-v", f"{TRAB.parent.parent / 'bancada/docker'}/phx-a:/dados", "phxsql:0.18.0")
    time.sleep(4)

    c = Cliente(6500)
    c.call({"op": "login", "usuario": "adm", "senha": SENHA_PHX})
    print("\n== a ligacao, com a senha por variavel ==")
    r = c.call({"op": "dblink_salvar", "nome": "erp", "motor": "mysql",
                "host": "erp-mariadb", "porta": 3306, "usuario": "leitor",
                "senha_env": "ERP_SENHA", "database": "erp"})
    print("  salvar:", r.get("ok"))
    t0 = time.time()
    r = c.call({"op": "dblink_testar", "nome": "erp"})
    d = r.get("resultado", {})
    print(f"  testar: {r.get('ok')} · {d.get('versao')} · usuario {d.get('usuario_efetivo')}"
          f" · {time.time() - t0:.2f}s")

    print("\n== o que o phx-a enxerga LA ==")
    print("  bancos :", c.call({"op": "dblink_bancos", "dblink": "erp"})["resultado"]["bancos"])
    tabs = c.call({"op": "dblink_tabelas", "dblink": "erp", "database": "erp"})["resultado"]["tabelas"]
    print("  tabelas:", [(t["nome"], t["registros_estimados"]) for t in tabs])

    print("\n== o DADO atravessando ==")
    for l in c.call({"op": "dblink_ler", "dblink": "erp", "tabela": "fornecedores",
                     "limite": 10})["resultado"]["linhas"]:
        print("   ", l)

    print("\n== JOIN + GROUP BY rodando LA, resultado chegando aqui ==")
    for l in c.call({"op": "dblink_consultar", "dblink": "erp", "limite": 10,
                     "sql": "SELECT f.razao, COUNT(n.id) AS notas, SUM(n.valor) AS total "
                            "FROM fornecedores f LEFT JOIN notas n ON n.fornecedor_id=f.id "
                            "GROUP BY f.razao ORDER BY f.razao"})["resultado"]["linhas"]:
        print("   ", l)

    print("\n== a senha vaza na ficha? ==")
    lig = c.call({"op": "dblink"})["resultado"]["ligacoes"]
    for l in lig:
        if l["nome"] == "erp":
            print(f"   senha={l['senha']!r}  senha_env={l['senha_env']!r}")

    print("\n== e o phx-a continua com a base dele ==")
    print("   bancos:", c.call({"op": "bancos"})["resultado"])
    return 0

if __name__ == "__main__":
    sys.exit(principal())
