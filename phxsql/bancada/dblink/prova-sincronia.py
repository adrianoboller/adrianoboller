#!/usr/bin/env python3
"""A prova da sincronia de tabelas primas, contra um MySQL(R) de verdade.

    python3 bancada/dblink/prova-sincronia.py

O que ela prova, na ordem -- e cada passo tem o resultado esperado escrito
ANTES de rodar, que e o que separa prova de demonstracao:

1. `dblink_ligar` cria a tabela local espelhando a remota (tipos, chave);
2. a primeira rodada PUXA tudo (5 novas, 0 conflito);
3. linha nova AQUI vai para la; linha nova LA vem para ca;
4. a MESMA linha mudada dos dois lados: o DONO vence (aqui);
5. exclusao NAO viaja: a linha apagada aqui REAPARECE, vinda de la --
   e o limite documentado, e a prova confere que ele e verdade;
6. a rodada e reentravel: rodar de novo sem mudanca da 0/0/0;
7. o job faz a rodada sozinho.

Ela sobe o proprio phxsqld, e por que
-------------------------------------
Ate 05/09/2026 esta prova falava com o `phxsqld` do DEMO, na porta 5599 --
um servidor levantado A MAO, que nao e desta bancada. Enquanto o demo esteve
no ar isso pareceu barato; no dia em que ele nao estava, a prova morreu com
um `ConnectionRefusedError` cru, sem dizer o que faltava. Bancada que so roda
quando alguem lembrou de subir um servidor a mao nao e bancada -- e as tres
irmas desta pasta ja subiam o proprio servidor. Hoje esta tambem sobe: porta
propria, token proprio, morta pelo PID, e o `adriano/demo123` do roteiro nasce
no `config.json` para que os passos abaixo continuem os mesmos.

O `mysqld` ela NAO sobe nem derruba -- e a mesma decisao da `prova-mysql.py`:
derrubar o banco de outra frente na mesma maquina custa mais do que a prova
vale. Se ele nao estiver no ar, ela diz o comando e para.
"""
import atexit, json, os, shutil, socket, subprocess, sys, time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
PHXSQLD = Path(os.environ.get("PHX_PHXSQLD", RAIZ / "target/release/phxsqld"))
TRABALHO = Path(os.environ.get("PHX_TRABALHO", f"/tmp/phx-dblink-sinc-{os.getpid()}"))
PORTA = int(os.environ.get("PORTA", "7493"))
TOKEN = "prova-sincronia"
SENHA_ROOT = "prova-sinc-8080"
# O roteiro dos passos 4 e 7 fala em nome do `adriano`, e o job guarda o dono
# da corrida. O usuario nasce no config para que os passos nao mudem.
USUARIO, SENHA = "adriano", "demo123"

MY_BASE, MY_USUARIO, MY_SENHA = "crm", "phx", "ponte123"
MY_HOST, MY_PORTA = "127.0.0.1", 3306

# O esquema da base da prova. Receita que so existe dentro do script morre com
# a sessao, entao ela esta tambem no LEIA-ME desta pasta.
ESQUEMA = """
CREATE TABLE IF NOT EXISTS clientes (
  id     BIGINT NOT NULL,
  nome   VARCHAR(60) NOT NULL,
  cidade VARCHAR(30),
  limite DECIMAL(12,2),
  desde  DATE,
  PRIMARY KEY (id),
  KEY porCidade (cidade)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
"""
SEMENTE = """
DELETE FROM clientes;
INSERT INTO clientes VALUES
  (1,'Adriano Boller','Curitiba-PR',15000.00,'2019-03-12'),
  (2,'Mercearia Blumenau','Blumenau',4200.50,'2020-11-02'),
  (3,'Posto Itajai','Itajai',9800.00,'2021-06-30'),
  (4,'Padaria Joinville','Joinville',1250.75,'2022-01-15'),
  (5,'Auto Pecas Chapeco','Chapeco',7600.00,'2023-08-09');
"""


def mysql(sql, base=MY_BASE):
    subprocess.run(["mysql", "--default-character-set=utf8mb4", base, "-e", sql],
                   check=True, capture_output=True)


def mysql_uma(sql, base=MY_BASE):
    r = subprocess.run(["mysql", "--default-character-set=utf8mb4", "-N", base,
                        "-e", sql], check=True, capture_output=True, text=True)
    return r.stdout.strip()


def hash_da_senha(senha):
    r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


def sobe_o_phxsqld():
    """Sobe o servidor desta prova e devolve o Popen. Morre pelo PID, e so ele."""
    shutil.rmtree(TRABALHO, ignore_errors=True)
    (TRABALHO / "dados").mkdir(parents=True)
    (TRABALHO / "config.json").write_text(json.dumps({
        "base": "dados",
        "bind": f"127.0.0.1:{PORTA}",
        "token": TOKEN,
        "web": {"ligado": False},
        "root": {"id": 1, "nome": "root", "login": "root",
                 "senha_hash": hash_da_senha(SENHA_ROOT)},
        # `supervisor` porque os passos administram: gravam a ligacao do
        # dblink, criam a tabela espelho e salvam o job.
        "usuarios": [{"id": 2, "nome": "Adriano", "login": USUARIO,
                      "supervisor": True,
                      "senha_hash": hash_da_senha(SENHA)}],
    }, indent=1))
    log = open(TRABALHO / "servidor.log", "a")
    proc = subprocess.Popen([str(PHXSQLD)], cwd=TRABALHO, stdout=log,
                            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(80):
        time.sleep(0.25)
        try:
            socket.create_connection(("127.0.0.1", PORTA), timeout=2).close()
            return proc, log
        except OSError:
            pass
    raise SystemExit(f"o phxsqld nao subiu; veja {TRABALHO / 'servidor.log'}")


if not PHXSQLD.exists():
    sys.exit(f"nao achei {PHXSQLD}.\nRode `cargo build --release` antes.")
try:
    socket.create_connection((MY_HOST, MY_PORTA), timeout=3).close()
except OSError:
    sys.exit(
        f"o MySQL nao esta no ar em {MY_HOST}:{MY_PORTA}.\n"
        "  service mysql start\n"
        "Esta prova NAO sobe nem derruba o mysqld: derrubar o banco de outra\n"
        "frente na mesma maquina custa mais do que a prova vale."
    )
try:
    subprocess.run(["mysql", "-e",
                    f"CREATE DATABASE IF NOT EXISTS {MY_BASE} "
                    "CHARACTER SET utf8mb4;"
                    f"CREATE USER IF NOT EXISTS '{MY_USUARIO}'@'{MY_HOST}' "
                    f"IDENTIFIED WITH mysql_native_password BY '{MY_SENHA}';"
                    f"ALTER USER '{MY_USUARIO}'@'{MY_HOST}' "
                    f"IDENTIFIED WITH mysql_native_password BY '{MY_SENHA}';"
                    f"GRANT ALL ON {MY_BASE}.* TO '{MY_USUARIO}'@'{MY_HOST}';"
                    "FLUSH PRIVILEGES;"], check=True, capture_output=True)
    mysql(ESQUEMA)
    # A semente e reposta em TODA corrida, e nao so quando a tabela falta: o
    # passo 4 deixa a linha 1 com o valor do dono e o 5 devolve a 2, entao uma
    # corrida interrompida no meio deixa o lado de la fora do comeco conhecido
    # -- e a prova seguinte passaria a medir o resto da anterior.
    mysql(SEMENTE)
except subprocess.CalledProcessError as e:
    sys.exit(f"nao consegui montar a base `{MY_BASE}` da prova: "
             f"{e.stderr.decode(errors='replace')[:300]}\n"
             "O SQL esta no LEIA-ME desta pasta; rode-o como root e chame de novo.")

PROC, LOG = sobe_o_phxsqld()


def _derruba():
    PROC.terminate()
    try:
        PROC.wait(timeout=20)
    except subprocess.TimeoutExpired:
        PROC.kill()
        PROC.wait()
    LOG.close()
    shutil.rmtree(TRABALHO, ignore_errors=True)


atexit.register(_derruba)

s = socket.create_connection(("127.0.0.1", PORTA), timeout=60); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token", TOKEN)
    f.write((json.dumps(p) + "\n").encode()); f.flush()
    r = json.loads(f.readline().decode())
    if not r.get("ok"):
        sys.exit(f"FALHOU {p['op']}: {r.get('erro')}")
    return r.get("resultado", r)

def sincroniza():
    return fala({"op": "dblink_sincronizar", "dblink": "crm"})["sincronizadas"][0]

def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok ' if ok else 'ERRO'} {rotulo}: {visto}" + ("" if ok else f" (esperava {esperado})"))
    if not ok:
        sys.exit(1)

fala({"op": "login", "usuario": USUARIO, "senha": SENHA})

# O comeco conhecido: 5 linhas la, ligacao gravavel, espelho local limpo.
mysql("DELETE FROM clientes WHERE id > 5")
mysql("UPDATE clientes SET cidade='Blumenau', limite=4200.50 WHERE id=2")
fala({"op": "dblink_salvar", "nome": "crm", "motor": "mysql", "host": "127.0.0.1",
      "porta": 3306, "usuario": "phx", "senha": "ponte123", "database": "crm",
      "somente_leitura": False})
fala({"op": "excluir_tabela", "database": "espelho", "tabela": "clientes", "de_vez": True}) \
    if False else None

print("== 1. ligar cria a tabela local espelhando a remota ==")
r = fala({"op": "dblink_ligar", "dblink": "crm",
          "tabelas": [{"remota": "clientes", "local_database": "espelho",
                        "sentido": "dois", "dono": "aqui"}]})["ligadas"][0]
confere("chave detectada", r["chave"], "id")

print("== 2. a primeira rodada puxa tudo ==")
r = sincroniza()
confere("puxadas novas", r["puxadas_novas"], 5)
confere("conflitos", r["conflitos"], 0)

print("== 3. linha nova de cada lado atravessa ==")
fala({"op": "inserir", "database": "espelho", "tabela": "clientes",
      "valores": {"id": 100, "nome": "Novo Daqui", "cidade": "Curitiba",
                   "limite": "10.00", "desde": "2026-08-29"}})
mysql("INSERT INTO clientes VALUES (6,'Novo De La','Lages',55.00,'2026-08-29')")
r = sincroniza()
confere("veio de la", r["puxadas_novas"], 1)
confere("foi para la", r["empurradas"], 1)
confere("id 100 chegou la", mysql_uma("SELECT nome FROM clientes WHERE id=100"), "Novo Daqui")

print("== 4. conflito: o dono (aqui) vence ==")
mysql("UPDATE clientes SET cidade='ERRADA' WHERE id=1")
# O rowid local se ACHA pela chave -- supor que a ordem da puxada e a ordem
# dos ids foi o primeiro defeito que esta prova pegou, e era da prova.
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [1]})
rowid = achado["linhas"][0]["rowid"] if "linhas" in achado else achado["rowids"][0]
fala({"op": "atualizar", "database": "espelho", "tabela": "clientes", "rowid": rowid,
      "valores": {"id": 1, "nome": "Adriano Boller", "cidade": "Curitiba-PR",
                   "limite": "15000.00", "desde": "2019-03-12"}})
r = sincroniza()
confere("um conflito visto", r["conflitos"], 1)
confere("a linha do dono venceu la", mysql_uma("SELECT cidade FROM clientes WHERE id=1"), "Curitiba-PR")

print("== 5. exclusao NAO viaja: a linha apagada reaparece ==")
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [2]})
rowid = achado["linhas"][0]["rowid"] if "linhas" in achado else achado["rowids"][0]
fala({"op": "excluir", "database": "espelho", "tabela": "clientes", "rowid": rowid,
      "fisico": True, "motivo": "prova do limite documentado"})
r = sincroniza()
confere("ela voltou de la", r["puxadas_novas"], 1)

print("== 6. reentravel: sem mudanca, rodada vazia ==")
r = sincroniza()
confere("nada a puxar", r["puxadas_novas"] + r["puxadas_alteradas"], 0)
confere("nada a empurrar", r["empurradas"], 0)
confere("sete iguais", r["iguais"], 7)

print("== 7. o job roda a sincronia sozinho ==")
fala({"op": "job_salvar", "nome": "sincronia-crm",
      "descricao": "convergencia com o MySQL(R) da bancada",
      "cada_minutos": 5, "ligado": True, "usuario": USUARIO,
      "pedido": {"op": "dblink_sincronizar", "dblink": "crm"}})
mysql("INSERT INTO clientes VALUES (7,'Chegou Pelo Job','Itajai',1.00,'2026-08-29')")
r = fala({"op": "job_rodar", "nome": "sincronia-crm"})
corrida = r.get("resultado", r)
print(f"  ok  job rodou: {json.dumps(corrida, ensure_ascii=False)[:100]}")
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [7]})
confere("a linha do job chegou aqui", achado["linhas"][0]["nome"], "Chegou Pelo Job")

print("\nPROVA COMPLETA: os dois lados convergem, o dono vence, o limite da")
print("exclusao e real, e o job faz a rodada sozinho -- cada afirmacao acima")
print("foi conferida contra o resultado, nao so impressa.")
