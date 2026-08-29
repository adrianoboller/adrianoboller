#!/usr/bin/env python3
"""A prova do aviso de jobs por e-mail, com SMTP falso e servidor proprio.

O que ela prova, na ordem -- e cada passo tem o resultado esperado escrito
ANTES de rodar, que e o que separa prova de demonstracao:

1. job que roda bem: o estado vira "ok" e NENHUM e-mail sai;
2. job cujo pedido falha: estado "falhou" e UM e-mail com o motivo -- e
   rodar de novo dentro do silencio NAO manda o segundo (repetir_horas);
3. job LIGADO num processo sem relogio (ele so sobe no arranque, e este
   arranque nao tinha job ligado): o vigia ve o PARADO e avisa por e-mail;
4. o mesmo roteiro num servidor SEM bloco de e-mail: mesmos eventos,
   NENHUMA conexao SMTP -- guarda nova entra pedida, nao imposta;
5. e o corte mais fino da mesma regra: servidor COM e-mail ligado (o aviso
   de disco de sempre) mas SEM "avisar_jobs" -- job falha e NENHUM e-mail
   sai, porque quem configurou e-mail para o disco nao pediu o de jobs.

O passo 3 espera de verdade a volta do vigia (60 s), entao a prova inteira
leva uns 3 minutos. E deliberado: encurtar o relogio para o teste seria
provar outro relogio.

Sobe um phxsqld PROPRIO em 5303 (dados) / 5703 (web) e um SMTP falso em
porta livre, e mata SO os processos que criou. Requisito:
    cargo build -p phxsql-server
"""

import base64
import json
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]  # .../phxsql
BIN = RAIZ / "target" / "debug" / "phxsqld"
PORTA_DADOS = 5303
PORTA_WEB = 5703
TOKEN = "prova-jobs"
SENHA = "prova123"


# ------------------------------------------------------------ SMTP falso
class SmtpFalso(threading.Thread):
    """Um rele de mentira em socket puro: aceita tudo e guarda o que chegou.

    Nao valida nada de proposito -- o que interessa e capturar o RCPT e o
    corpo para conferir do lado de ca. `conexoes` conta ate as conversas que
    nao viraram mensagem, porque no passo 4 o certo e ZERO conexao."""

    def __init__(self):
        super().__init__(daemon=True)
        self.soquete = socket.socket()
        self.soquete.bind(("127.0.0.1", 0))
        self.soquete.listen(8)
        self.porta = self.soquete.getsockname()[1]
        self.mensagens = []
        self.conexoes = 0
        self.trava = threading.Lock()

    def run(self):
        while True:
            try:
                cliente, _ = self.soquete.accept()
            except OSError:
                return
            with self.trava:
                self.conexoes += 1
            threading.Thread(target=self.atender, args=(cliente,), daemon=True).start()

    def atender(self, cliente):
        f = cliente.makefile("rwb")

        def diz(linha):
            f.write((linha + "\r\n").encode())
            f.flush()

        diz("220 smtp-falso pronto")
        rcpt, de = [], ""
        try:
            while True:
                linha = f.readline().decode("utf-8", "replace").strip()
                if not linha:
                    break
                verbo = linha.split(":")[0].split(" ")[0].upper()
                if verbo == "EHLO":
                    f.write(b"250-smtp-falso\r\n250 OK\r\n")
                    f.flush()
                elif verbo == "HELO":
                    diz("250 OK")
                elif verbo == "MAIL":
                    de = linha.split(":", 1)[1].strip(" <>")
                    diz("250 OK")
                elif verbo == "RCPT":
                    rcpt.append(linha.split(":", 1)[1].strip(" <>"))
                    diz("250 OK")
                elif verbo == "DATA":
                    diz("354 manda")
                    cru = []
                    while True:
                        l = f.readline().decode("utf-8", "replace")
                        if l.rstrip("\r\n") == ".":
                            break
                        cru.append(l)
                    with self.trava:
                        self.mensagens.append(self.analisar(de, rcpt[:], "".join(cru)))
                    rcpt = []
                    diz("250 OK guardei")
                elif verbo == "QUIT":
                    diz("221 tchau")
                    break
                else:
                    diz("250 OK")
        except OSError:
            pass
        finally:
            cliente.close()

    @staticmethod
    def analisar(de, para, cru):
        """Cabecalho e corpo decodificados -- o corpo vem em base64 (RFC 2045)
        e o assunto com acento em palavra codificada (RFC 2047)."""
        cabeca, _, corpo = cru.partition("\r\n\r\n")
        if not corpo:
            cabeca, _, corpo = cru.partition("\n\n")
        assunto = ""
        for linha in cabeca.splitlines():
            if linha.lower().startswith("subject:"):
                assunto = linha.split(":", 1)[1].strip()
                if assunto.startswith("=?UTF-8?B?"):
                    assunto = base64.b64decode(
                        assunto[len("=?UTF-8?B?"):].rstrip("?=")
                    ).decode()
        try:
            texto = base64.b64decode("".join(corpo.split())).decode()
        except Exception:
            texto = corpo
        return {"de": de, "para": para, "assunto": assunto, "corpo": texto}

    def espera_mensagens(self, quantas, ate_s):
        fim = time.time() + ate_s
        while time.time() < fim:
            with self.trava:
                if len(self.mensagens) >= quantas:
                    return True
            time.sleep(0.5)
        return False


# ------------------------------------------------------- servidor proprio
def hash_da_senha():
    r = subprocess.run([BIN, "--senha", SENHA], check=True, capture_output=True, text=True)
    return r.stdout.split('"senha_hash": "')[1].split('"')[0]


def escrever_config(pasta, com_email, porta_smtp, avisar_jobs=True):
    alertas = {
        # `ligado` fica FALSO de proposito: o vigia de DISCO nao entra nesta
        # prova, e o aviso de jobs tem de andar sem ele.
        "email": {
            "ligado": True,
            "avisar_jobs": avisar_jobs,
            "servidor": "127.0.0.1",
            "porta": porta_smtp,
            "de": "phxsql@prova.local",
            "para": ["adriano@prova.local"],
            "timeout_s": 5,
        }
    }
    if not avisar_jobs:
        # O corte fino do passo 5: o bloco de e-mail do aviso de disco, como
        # ele ja existia antes desta funcionalidade -- sem a chave nova.
        del alertas["email"]["avisar_jobs"]
    config = {
        "bind": f"127.0.0.1:{PORTA_DADOS}",
        "base": str(pasta / "dados"),
        "token": TOKEN,
        "log_acessos": str(pasta / "acessos.log"),
        "seguranca": {"blacklist": str(pasta / "blacklist.json")},
        "dblink": str(pasta / "dblink.json"),
        "jobs": str(pasta / "jobs.json"),
        "web": {"ligado": True, "bind": f"127.0.0.1:{PORTA_WEB}"},
        "root": {"id": 1, "login": "root", "nome": "Root da prova",
                 "senha_hash": hash_da_senha()},
    }
    if com_email:
        config["alertas"] = alertas
    caminho = pasta / "config.json"
    caminho.write_text(json.dumps(config, indent=2))
    return caminho


def subir_servidor(config):
    log = open(config.parent / "phxsqld.log", "w")
    proc = subprocess.Popen([BIN, "--config", config], stdout=log, stderr=log)
    fim = time.time() + 10
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", PORTA_DADOS), 0.3).close()
            return proc
        except OSError:
            if proc.poll() is not None:
                sys.exit(f"phxsqld morreu no arranque; veja {config.parent}/phxsqld.log")
            time.sleep(0.1)
    proc.terminate()
    sys.exit("phxsqld nao abriu a porta 5303 em 10 s")


class Cliente:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORTA_DADOS), 5)
        self.f = self.s.makefile("rwb")

    def fala(self, pedido, pode_falhar=False):
        pedido.setdefault("token", TOKEN)
        self.f.write((json.dumps(pedido) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        if not r.get("ok") and not pode_falhar:
            sys.exit(f"FALHOU {pedido['op']}: {r.get('erro')}")
        return r.get("resultado", r)


def confere(rotulo, visto, esperado=True):
    ok = visto == esperado
    print(f"  {'ok ' if ok else 'ERRO'} {rotulo}" + ("" if ok else f": {visto!r} (esperava {esperado!r})"))
    if not ok:
        sys.exit(1)


def ficha(c, nome):
    lista = c.fala({"op": "jobs"})["jobs"]
    return next(j for j in lista if j["nome"] == nome)


# ================================================================= prova
def fase_com_aviso(smtp):
    pasta = Path(tempfile.mkdtemp(prefix="phxsql-prova-jobs-A-"))
    proc = subir_servidor(escrever_config(pasta, True, smtp.porta))
    try:
        c = Cliente()
        c.fala({"op": "login", "usuario": "root", "senha": SENHA})

        # Os dois primeiros jobs ficam LIGADOS com agenda folgada: a corrida
        # que cada passo dispara os tira do "vencido", entao o vigia do passo
        # 3 nao os confunde com parados -- so o "abandonado" fica devendo.
        print("== 1. job que roda bem: estado ok, nenhum e-mail ==")
        c.fala({"op": "job_salvar", "job": {"nome": "sereno", "usuario": "root",
                "ligado": True, "cada_minutos": 60, "pedido": {"op": "ping"}}})
        r = c.fala({"op": "job_rodar", "nome": "sereno"})
        confere("a corrida deu certo", r["ok"])
        f = ficha(c, "sereno")
        confere("estado", f["estado"], "ok")
        confere("a ultima corrida ficou na ficha", f["ultima"]["ok"])
        confere("ha proxima prevista", f["proximo_ms"] is not None)
        confere("nenhum e-mail para job que rodou", len(smtp.mensagens), 0)

        print("== 2. job que falha: estado falhou, UM e-mail com o motivo ==")
        c.fala({"op": "job_salvar", "job": {"nome": "quebrado", "usuario": "root",
                "ligado": True, "cada_minutos": 60,
                "pedido": {"op": "varrer", "database": "NaoExiste", "tabela": "Nada"}}})
        r = c.fala({"op": "job_rodar", "nome": "quebrado"})
        confere("a corrida falhou (o job, nao o pedido)", r["ok"], False)
        f = ficha(c, "quebrado")
        confere("estado", f["estado"], "falhou")
        confere("a falha esta na ultima corrida", f["ultima"]["ok"], False)
        confere("o e-mail de falha chegou", smtp.espera_mensagens(1, 10))
        m = smtp.mensagens[0]
        confere("para quem o config manda", m["para"], ["adriano@prova.local"])
        confere("o assunto nomeia o job", "quebrado" in m["assunto"] and "falhou" in m["assunto"])
        confere("o corpo traz o motivo", "NaoExiste" in m["corpo"])
        confere("nenhuma senha na mensagem",
                SENHA not in m["corpo"] and "pbkdf2" not in m["corpo"])

        print("== 2b. a mesma falha dentro do silencio NAO repete o e-mail ==")
        c.fala({"op": "job_rodar", "nome": "quebrado"})
        time.sleep(3)
        confere("continua um e-mail so", len(smtp.mensagens), 1)

        print("== 3. job ligado sem relogio: o vigia avisa o PARADO ==")
        c.fala({"op": "job_salvar", "job": {"nome": "abandonado", "usuario": "root",
                "ligado": True, "cada_minutos": 1, "pedido": {"op": "ping"}}})
        f = ficha(c, "abandonado")
        confere("estado", f["estado"], "nunca_rodou")
        confere("a ficha ja diz parado", f["parado"])
        confere("e diz que nao ha relogio", c.fala({"op": "jobs"})["relogio_no_ar"], False)
        print("  ... esperando a volta do vigia (ate 75 s)")
        confere("o e-mail de parado chegou", smtp.espera_mensagens(2, 75))
        m = smtp.mensagens[1]
        confere("o assunto diz sem rodar", "sem rodar" in m["assunto"])
        confere("o corpo nomeia o job", "abandonado" in m["corpo"])
        confere("o corpo explica o relogio", "relógio" in m["corpo"])
    finally:
        proc.terminate()
        proc.wait()
    print(f"  (arquivos da fase A em {pasta})")
    return pasta


def fase_sem_aviso(smtp):
    pasta = Path(tempfile.mkdtemp(prefix="phxsql-prova-jobs-B-"))
    proc = subir_servidor(escrever_config(pasta, False, smtp.porta))
    try:
        antes = smtp.conexoes
        c = Cliente()
        c.fala({"op": "login", "usuario": "root", "senha": SENHA})

        print("== 4. sem bloco de e-mail: mesmos eventos, NENHUMA conexao SMTP ==")
        c.fala({"op": "job_salvar", "job": {"nome": "quebrado", "usuario": "root",
                "cada_minutos": 60,
                "pedido": {"op": "varrer", "database": "NaoExiste", "tabela": "Nada"}}})
        r = c.fala({"op": "job_rodar", "nome": "quebrado"})
        confere("a falha continua acontecendo", r["ok"], False)
        confere("e continua no historico", ficha(c, "quebrado")["ultima"]["ok"], False)
        c.fala({"op": "job_salvar", "job": {"nome": "abandonado", "usuario": "root",
                "ligado": True, "cada_minutos": 1, "pedido": {"op": "ping"}}})
        confere("a ficha continua dizendo parado", ficha(c, "abandonado")["parado"])
        confere("a tela sabe que o aviso nao existe",
                c.fala({"op": "jobs"})["aviso_email"]["ligado"], False)
        print("  ... esperando a janela em que a fase A avisou (75 s)")
        time.sleep(75)
        confere("nenhuma conexao SMTP nova", smtp.conexoes - antes, 0)
    finally:
        proc.terminate()
        proc.wait()


def fase_sem_pedir(smtp):
    """E-mail LIGADO (o aviso de disco de sempre), `avisar_jobs` ausente."""
    pasta = Path(tempfile.mkdtemp(prefix="phxsql-prova-jobs-C-"))
    proc = subir_servidor(escrever_config(pasta, True, smtp.porta, avisar_jobs=False))
    try:
        antes = smtp.conexoes
        c = Cliente()
        c.fala({"op": "login", "usuario": "root", "senha": SENHA})
        print("== 5. e-mail ligado sem avisar_jobs: job falha, NENHUM e-mail ==")
        c.fala({"op": "job_salvar", "job": {"nome": "quebrado", "usuario": "root",
                "cada_minutos": 60,
                "pedido": {"op": "varrer", "database": "NaoExiste", "tabela": "Nada"}}})
        r = c.fala({"op": "job_rodar", "nome": "quebrado"})
        confere("a falha continua acontecendo", r["ok"], False)
        aviso = c.fala({"op": "jobs"})["aviso_email"]
        confere("a tela diz: e-mail sim, aviso de jobs nao",
                (aviso["email_ligado"], aviso["ligado"]), (True, False))
        time.sleep(5)
        confere("nenhuma conexao SMTP: quem pediu disco recebe disco",
                smtp.conexoes - antes, 0)
    finally:
        proc.terminate()
        proc.wait()


if __name__ == "__main__":
    if not BIN.exists():
        sys.exit(f"falta o binario {BIN}: rode antes  cargo build -p phxsql-server")
    smtp = SmtpFalso()
    smtp.start()
    print(f"SMTP falso em 127.0.0.1:{smtp.porta}")
    # `--so-5` roda apenas o corte fino -- e o passo que faz papel de prova
    # real: com o portao do avisar_jobs removido do servidor, ele FALHA.
    if "--so-5" in sys.argv:
        fase_sem_pedir(smtp)
        print("\npasso 5 conferiu.")
        sys.exit(0)
    pasta_a = fase_com_aviso(smtp)
    fase_sem_aviso(smtp)
    fase_sem_pedir(smtp)
    print("\nPROVA COMPLETA: os 5 passos conferiram.")
    print(f"Para a tela: {BIN} --config {pasta_a}/config.json  (web em 127.0.0.1:{PORTA_WEB})")
