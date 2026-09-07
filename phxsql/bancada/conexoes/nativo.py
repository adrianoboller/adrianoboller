#!/usr/bin/env python3
"""A conexao NATIVA do PhxSql: soquete TCP cru + JSON Lines, sem cliente nenhum.

    python3 bancada/conexoes/nativo.py

Sobe um `phxsqld` proprio (porta 6400, faixa da frente F4), mostra as TRES
formas de login que o `docs/SEGURANCA.md` descreve -- desafio-resposta,
Base64 e texto puro -- e faz um CRUD completo pelo protocolo. Por fim chama o
console oficial (`phxsqlcmd`) contra o MESMO servidor, para provar que console
e "conexao nativa a mao" sao o mesmo protocolo por dois caminhos.

Nunca usa `pkill`: mata so o processo que ele mesmo criou, pelo PID guardado.
"""
import hashlib
import hmac
import json
import os
import secrets
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
PHXSQLD = RAIZ / "target/release/phxsqld"
PHXSQLCMD = RAIZ / "target/release/phxsqlcmd"
TRABALHO = Path(f"/tmp/phx-f4-{os.getpid()}-nativo")

PORTA = 6400
TOKEN = "token-da-porta"
USUARIO = "adriano"
SENHA = "correta-horse-battery"

falhas = []


def afirma(rotulo, condicao, visto=""):
    ok = bool(condicao)
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}" + (f"  -- {visto}" if visto != "" else ""))
    if not ok:
        falhas.append(rotulo)


def hash_da_senha(senha):
    """A linha pronta do config, gerada pelo proprio phxsqld -- o sal muda a
    cada rodada, entao colar um hash fixo aqui seria colar um sal fixo."""
    r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


class Servidor:
    def __init__(self):
        shutil.rmtree(TRABALHO, ignore_errors=True)
        (TRABALHO / "dados").mkdir(parents=True)
        (TRABALHO / "config.json").write_text(json.dumps({
            "base": "dados",
            "bind": f"127.0.0.1:{PORTA}",
            "token": TOKEN,
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": USUARIO,
                     "senha_hash": hash_da_senha(SENHA)},
            "usuarios": [],
        }, indent=1))
        self.log = open(TRABALHO / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=TRABALHO, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
        )
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", PORTA), timeout=2).close()
                print(f"phxsqld pid {self.proc.pid} na porta {PORTA}")
                return
            except OSError:
                pass
        raise SystemExit(f"o servidor nao subiu; veja {TRABALHO / 'servidor.log'}")

    def parar(self):
        # Pelo PID, e so ele -- nunca pkill -f, que derrubaria servidor de
        # outra frente na mesma maquina.
        self.proc.terminate()
        try:
            self.proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Fio:
    """Um soquete cru + JSON Lines -- a conexao nativa, sem lib do PhxSql."""

    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORTA), timeout=10)
        self.f = self.s.makefile("rwb")

    def bruto(self, **kw):
        kw.setdefault("token", TOKEN)
        linha = json.dumps(kw)
        print(f"      -> {linha}")
        self.f.write((linha + "\n").encode())
        self.f.flush()
        resp = self.f.readline().decode().rstrip("\n")
        print(f"      <- {resp}")
        return json.loads(resp)

    def pedir(self, **kw):
        r = self.bruto(**kw)
        if not r.get("ok"):
            raise SystemExit(f"{kw.get('op')} recusado: {r}")
        return r.get("resultado")

    def fechar(self):
        self.f.close()
        self.s.close()


def prova_desafio_resposta(usuario, senha):
    """O calculo do docs/SEGURANCA.md Sec.2, com so a std do Python."""
    fio = Fio()
    print("\n--- 1. Login por DESAFIO-RESPOSTA (a senha nunca sai da maquina) ---")
    d = fio.pedir(op="desafio", usuario=usuario)
    sal = bytes.fromhex(d["sal"])
    iteracoes = d["iteracoes"]
    nonce = d["nonce"]
    nonce_cliente = secrets.token_hex(16)
    dk = hashlib.pbkdf2_hmac("sha256", senha.encode(), sal, iteracoes, 32)
    msg = f"{nonce},{nonce_cliente},{usuario}".encode()
    prova = hmac.new(dk, msg, hashlib.sha256).hexdigest()
    r = fio.pedir(op="login", usuario=usuario, nonce_cliente=nonce_cliente, prova=prova)
    afirma("logou sem a senha trafegar", True, r)
    fio.fechar()


def prova_login_base64(usuario, senha):
    fio = Fio()
    print("\n--- 2. Login por BASE64 (esconde do olho e do grep, nao da rede) ---")
    import base64
    r = fio.pedir(op="login", usuario_b64=base64.b64encode(usuario.encode()).decode(),
                  senha_b64=base64.b64encode(senha.encode()).decode())
    afirma("logou com usuario/senha em base64", True, r)
    fio.fechar()


def prova_login_texto_puro(usuario, senha):
    fio = Fio()
    print("\n--- 3. Login por TEXTO PURO (a menos segura das tres -- so para medir) ---")
    r = fio.pedir(op="login", usuario=usuario, senha=senha)
    afirma("logou com a senha em claro no pedido", True, r)
    fio.fechar()


def prova_crud():
    print("\n--- 4. CRUD completo pela conexao nativa, apos login por desafio-resposta ---")
    fio = Fio()
    d = fio.pedir(op="desafio", usuario=USUARIO)
    sal = bytes.fromhex(d["sal"])
    nonce_cliente = secrets.token_hex(16)
    dk = hashlib.pbkdf2_hmac("sha256", SENHA.encode(), sal, d["iteracoes"], 32)
    msg = f"{d['nonce']},{nonce_cliente},{USUARIO}".encode()
    prova = hmac.new(dk, msg, hashlib.sha256).hexdigest()
    fio.pedir(op="login", usuario=USUARIO, nonce_cliente=nonce_cliente, prova=prova)

    fio.pedir(op="criar_database", database="loja")
    fio.pedir(op="criar_tabela", database="loja", tabela="clientes",
              colunas=[{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                       {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                       {"nome": "cidade", "tipo": "Str(30)"}],
              indices=[{"nome": "pk_id", "colunas": ["id"], "unico": True, "primario": True}])
    ins = fio.pedir(op="inserir", database="loja", tabela="clientes",
                     valores={"id": 1, "nome": "Adriano Boller", "cidade": "Blumenau"})
    afirma("inseriu e devolveu o rowid", ins.get("rowid") == 1, ins)
    lido = fio.pedir(op="ler", database="loja", tabela="clientes", rowid=1)
    afirma("leu a linha de volta, sem mudar a caixa do dado",
           lido["nome"] == "Adriano Boller" and lido["cidade"] == "Blumenau",
           lido)
    sql = fio.pedir(op="sql", database="loja", texto="SELECT nome, cidade FROM clientes")
    afirma("SELECT pela mesma porta devolveu a linha",
           sql["linhas"] == [{"nome": "Adriano Boller", "cidade": "Blumenau"}], sql["linhas"])
    fio.fechar()


def prova_console():
    print("\n--- 5. O MESMO servidor, agora pelo console oficial phxsqlcmd ---")
    ambiente = dict(os.environ, PHXSQL_SENHA=SENHA)
    for comando in (
        "bancos",
        "tabelas database=loja",
        "SELECT nome, cidade FROM clientes",
    ):
        r = subprocess.run(
            [str(PHXSQLCMD), "--host", "127.0.0.1", "--porta", str(PORTA),
             "--token", TOKEN, "--usuario", USUARIO, "--database", "loja",
             "--comando", comando],
            capture_output=True, text=True, env=ambiente, timeout=20,
        )
        print(f"\n  $ phxsqlcmd ... --comando '{comando}'")
        for linha in r.stdout.rstrip("\n").splitlines():
            print(f"    {linha}")
        afirma(f"phxsqlcmd '{comando}' saiu com codigo 0", r.returncode == 0, r.returncode)


def main():
    for caminho in (PHXSQLD, PHXSQLCMD):
        if not caminho.exists():
            sys.exit(f"nao achei {caminho} -- os binarios ja precisam estar em target/release/")

    print("=== Conexao NATIVA do PhxSql -- soquete TCP + JSON Lines ===")
    servidor = Servidor()
    try:
        prova_desafio_resposta(USUARIO, SENHA)
        prova_login_base64(USUARIO, SENHA)
        prova_login_texto_puro(USUARIO, SENHA)
        prova_crud()
        prova_console()
    finally:
        servidor.parar()
        shutil.rmtree(TRABALHO, ignore_errors=True)

    print("\n" + ("=== TUDO OK ===" if not falhas else f"=== {len(falhas)} FALHA(S): {falhas} ==="))
    return 1 if falhas else 0


if __name__ == "__main__":
    raise SystemExit(main())
