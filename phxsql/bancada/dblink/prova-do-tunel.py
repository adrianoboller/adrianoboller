#!/usr/bin/env python3
"""O tunel do DbLink (pedido 378) MEDIDO contra PhxSql de verdade.

    python3 bancada/dblink/prova-do-tunel.py

A suite ja prova, pelo soquete, que o `{"op":"cifrar"}` e a PRIMEIRA linha que
sai do fio (`crates/phxsql-server/tests/dblink-phx-no-fio.rs`). O que ela nao
prova e que o outro lado FECHA o aperto, nem que o escape escrito volta a
falar claro com um servidor que nao o atende -- para isso e preciso um PhxSql
de verdade do outro lado.

O ORACULO, e por que ele nao e o "ok" da conexao
------------------------------------------------
Um dos tres servidores sobe com `cifra_fio.ligada: false`: ele RECUSA o aperto.
Se o DbLink estivesse indo em claro, ele conectaria igual e a prova nao mediria
nada -- seria o teste que passa por engano. E o servidor SURDO que transforma
"conectou" em medida:

    ligacao de fabrica  contra o servidor SURDO    -> tem de FALHAR
    "cifra": false      contra o MESMO servidor    -> tem de CONECTAR
    ligacao de fabrica  contra o servidor OUVINTE  -> tem de CONECTAR

O que ela sobe e o que ela nao mata
-----------------------------------
Tres `phxsqld` proprios, em portas proprias (17591-17593), mortos pelo PID --
nunca `pkill`, que derrubaria o servidor de outra frente na mesma maquina.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

RAIZ = Path("/home/user/adrianoboller/phxsql")
PHXSQLD = RAIZ / "target/release/phxsqld"
TRABALHO = Path(f"/tmp/phx-tunel-dblink-{os.getpid()}")
falhas = []


def hash_da_senha(senha):
    r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    return r.stdout.decode().split('"')[3]


class Phxsqld:
    def __init__(self, nome, porta, token, senha, cifra_fio):
        self.porta, self.token, self.senha = porta, token, senha
        self.base = TRABALHO / nome
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        (self.base / "config.json").write_text(json.dumps({
            "base": "dados",
            "bind": f"127.0.0.1:{porta}", "cifra_fio": cifra_fio,
            "token": token,
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": hash_da_senha(senha)},
            "usuarios": [],
        }, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", porta), timeout=2).close()
                return
            except OSError:
                pass
        raise SystemExit(f"o {nome} nao subiu; veja {self.base/'servidor.log'}")

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Cliente:
    def __init__(self, sv):
        self.token = sv.token
        self.s = socket.create_connection(("127.0.0.1", sv.porta), timeout=60)
        self.f = self.s.makefile("rwb")
        self.call({"op": "login", "usuario": "root", "senha": sv.senha})

    def bruto(self, d):
        d.setdefault("token", self.token)
        self.s.sendall((json.dumps(d) + "\n").encode())
        return json.loads(self.f.readline())

    def call(self, d):
        r = self.bruto(d)
        if not r.get("ok"):
            raise SystemExit(f"{d['op']} recusado: " + json.dumps(r)[:400])
        return r.get("resultado")


def afirma(rotulo, cond, visto):
    print(f"  {'ok  ' if cond else 'ERRO'} {rotulo}: {visto}")
    if not cond:
        falhas.append(rotulo)


def idade_do_binario():
    """Diz a IDADE do `phxsqld` contra o fonte, antes de medir qualquer coisa.

    Esta bancada nao compila -- por decisao, para nao competir com quem
    esta compilando na mesma maquina. O preco disso e que ela pode medir o
    PASSADO: em 23/09/2026 o papel F rodou-a com um `target/release/phxsqld`
    de uma compilacao anterior e leu TRES `ERRO` como defeito do motor,
    quando eram do binario velho. O script nao tinha como dizer a diferenca.

    Ele continua nao compilando. O que muda e que agora ele DIZ a idade, e
    quem le julga -- em vez de acreditar. E' a mesma lei que fez a bancada de
    carga perder uma rodada inteira de ganhos por chamar um `examples/` que
    `cargo build --release` nao recompila.
    """
    if not PHXSQLD.exists():
        print(f"!! {PHXSQLD} nao existe -- rode `cargo build --release` antes")
        sys.exit(1)
    bin_mt = PHXSQLD.stat().st_mtime
    fontes = [f for f in (RAIZ / "crates").rglob("*.rs")
              if "/target/" not in str(f)]
    mais_novo = max(fontes, key=lambda f: f.stat().st_mtime)
    atraso = mais_novo.stat().st_mtime - bin_mt
    quando = time.strftime("%d/%m/%Y %H:%M", time.localtime(bin_mt))
    print(f"== binario: {quando} ({len(fontes)} fontes conferidos)")
    if atraso > 0:
        print(f"!! ATENCAO: {mais_novo.relative_to(RAIZ)} e mais novo que o "
              f"binario em {int(atraso)}s -- esta corrida mede o PASSADO.")
        print("   Rode `cargo build --release` e repita, ou leia cada ERRO "
              "abaixo sabendo que ele pode ser do binario e nao do motor.")


def principal():
    idade_do_binario()
    surdo = Phxsqld("surdo", 17591, "tok-surdo", "senha-surda-123",
                    {"exigir": False, "ligada": False})
    ouvinte = Phxsqld("ouvinte", 17592, "tok-ouve", "senha-ouve-123",
                      {"exigir": False})
    casa = Phxsqld("casa", 17593, "tok-casa", "senha-casa-123",
                   {"exigir": False})
    try:
        c = Cliente(casa)

        print("\n-- o padrao: de fabrica a ligacao phxsql PEDE o tunel")
        c.call({"op": "dblink_salvar", "nome": "surdo", "motor": "phxsql",
                "host": "127.0.0.1", "porta": 17591, "token_remoto": "tok-surdo"})
        lig = c.call({"op": "dblink"})["ligacoes"][0]
        afirma("a ficha diz cifra=true sem ninguem ter escrito",
               lig.get("cifra") is True and lig.get("tem_pino") is False, lig.get("cifra"))
        r = c.bruto({"op": "dblink_testar", "dblink": "surdo"})
        afirma("contra o servidor que RECUSA o aperto, o DbLink nao entra",
               not r.get("ok"), json.dumps(r)[:400])
        afirma("e a recusa ensina o escape",
               "cifra" in json.dumps(r), json.dumps(r)[:200])

        print("\n-- o escape escrito: `\"cifra\": false` volta ao claro")
        c.call({"op": "dblink_salvar", "nome": "surdo", "motor": "phxsql",
                "host": "127.0.0.1", "porta": 17591, "cifra": False})
        r = c.bruto({"op": "dblink_testar", "dblink": "surdo"})
        afirma("com o escape escrito, o MESMO servidor responde",
               r.get("ok"), json.dumps(r)[:200])
        afirma("e o token sobreviveu a edicao que so mandou a cifra",
               r.get("ok"), "conectou")

        print("\n-- contra um servidor que atende o aperto, de fabrica entra")
        c.call({"op": "dblink_salvar", "nome": "ouve", "motor": "phxsql",
                "host": "127.0.0.1", "porta": 17592, "token_remoto": "tok-ouve"})
        r = c.bruto({"op": "dblink_testar", "dblink": "ouve"})
        afirma("o tunel sobe e a ligacao responde", r.get("ok"),
               json.dumps(r.get("resultado", r))[:200])

        print("\n-- o pino: chave errada derruba, chave certa passa")
        c.call({"op": "dblink_salvar", "nome": "ouve", "motor": "phxsql",
                "host": "127.0.0.1", "porta": 17592,
                "chave_do_fio": "aa" * 32})
        r = c.bruto({"op": "dblink_testar", "dblink": "ouve"})
        afirma("com pino ERRADO a conexao cai em vez de seguir",
               not r.get("ok"), json.dumps(r)[:240])
        lig = [x for x in c.call({"op": "dblink"})["ligacoes"]
               if x["nome"] == "ouve"][0]
        afirma("a ficha diz tem_pino e NAO mostra o pino",
               lig.get("tem_pino") is True and "aaaa" not in json.dumps(lig),
               json.dumps(lig)[:200])

        print("\n-- o disco: o padrao NAO e fossilizado, a decisao SIM")
        disco = json.loads((casa.base / "dblink.json").read_text())
        porn = {x["nome"]: x for x in disco["dblink"]}
        afirma("a ligacao com o escape guardou `cifra: false`",
               porn["surdo"].get("cifra") is False, json.dumps(porn["surdo"]))
        afirma("a ligacao de fabrica NAO guardou campo de cifra",
               "cifra" not in porn["ouve"], json.dumps(porn["ouve"]))
        afirma("e o pino foi guardado inteiro",
               porn["ouve"].get("chave_do_fio") == "aa" * 32,
               porn["ouve"].get("chave_do_fio"))

        print("\n-- salvar pela tela (sem pino, sem token) NAO apaga o pino")
        c.call({"op": "dblink_salvar", "nome": "ouve", "motor": "phxsql",
                "host": "127.0.0.1", "porta": 17592, "descricao": "editada"})
        disco = json.loads((casa.base / "dblink.json").read_text())
        porn = {x["nome"]: x for x in disco["dblink"]}
        afirma("o pino sobreviveu ao salvar pela tela",
               porn["ouve"].get("chave_do_fio") == "aa" * 32,
               porn["ouve"].get("chave_do_fio"))
        afirma("e a edicao pegou", porn["ouve"].get("descricao") == "editada",
               porn["ouve"].get("descricao"))

        print("\n-- motor alheio: recusa na DECLARACAO, nomeando o motor")
        for motor, campo in [("mysql", {"cifra": True}),
                             ("postgres", {"chave_do_fio": "bb" * 32})]:
            r = c.bruto({"op": "dblink_salvar", "nome": "fora", "motor": motor,
                         "host": "127.0.0.1", **campo})
            afirma(f"{motor} recusa {list(campo)[0]}",
                   not r.get("ok") and motor in json.dumps(r),
                   json.dumps(r)[:240])
    finally:
        for sv in (casa, ouvinte, surdo):
            sv.parar()
        shutil.rmtree(TRABALHO, ignore_errors=True)

    if falhas:
        print(f"\nFALHAS: {falhas}")
        sys.exit(1)
    print("\nas conferencias passaram todas.")


principal()
