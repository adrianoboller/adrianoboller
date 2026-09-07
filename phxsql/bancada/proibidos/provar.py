#!/usr/bin/env python3
"""Bateria dos COMANDOS PROIBIDOS: o servidor recusa, bloqueia o IP -- e avisa?

    flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
    python3 bancada/proibidos/provar.py

Pedido do dono, 07/09/2026: *«bateria de teste com comandos definidos como
proibidos para um banco x que devem ser negados pelo servidor avisando o
administrador por e-mail»*.

# Por que existe

O `config.json` tem `seguranca.comandos_proibidos` e `seguranca.bases_proibidas`
desde a 0.13. A recusa e o bloqueio do IP estavam medidos por teste unitario; o
E-MAIL ao administrador nunca esteve. Ler o codigo diz que `violacao_grave` so
chama `eprintln!`; esta bateria PROVA -- contra um rele SMTP de verdade, ainda
que falso, que conta as mensagens que chegam.

# O instrumento antes do veredito

Um SMTP falso que nao recebe nada nao prova ausencia de e-mail: prova que o
SMTP falso nao presta. Entao a parte 1 dispara um e-mail que o motor JA sabia
mandar -- o aviso de job que falhou -- e so depois disso a ausencia (ou a
presenca) na violacao grave vale como medida.

# Como roda

Sobe dois `phxsqld` de verdade (portas 6500 e 6501) e um SMTP falso (6510),
todos em 127.0.0.1. TRES servidores porque cada um se auto-bloqueia: 127.0.0.1
nao esta na whitelist, entao o comando proibido barra o proprio medidor -- que e
exatamente o comportamento que se quer provar. O terceiro sobe com
`avisar_seguranca` FALSO, para provar a outra metade da guarda: quem nao pediu
continua sendo bloqueado e nao recebe e-mail nenhum. Derruba tudo por PID no
fim.
"""

import base64
import json
import os
import shutil
import socket
import socketserver
import subprocess
import sys
import threading
import time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARIO = os.path.join(RAIZ, "target/release/phxsqld")
BASE = f"/tmp/phx-f5-{os.getpid()}"
PORTA_A = int(os.environ.get("PHX_F5_PORTA_A", "6500"))
PORTA_B = int(os.environ.get("PHX_F5_PORTA_B", "6501"))
PORTA_C = int(os.environ.get("PHX_F5_PORTA_C", "6502"))
PORTA_SMTP = int(os.environ.get("PHX_F5_PORTA_SMTP", "6510"))

PROIBIDOS = ["excluir_tabela", "reindexar"]
BASES_PROIBIDAS = ["financeiro"]

CAIXA = []
CAIXA_TRAVA = threading.Lock()


# --------------------------------------------------------------- SMTP falso
class Rele(socketserver.StreamRequestHandler):
    """Um SMTP do tamanho do que o `email.rs` fala: 220/250/354/250/221."""

    def handle(self):
        self.wfile.write(b"220 rele-falso ESMTP PhxSql-bancada\r\n")
        de, para, corpo = "", [], ""
        while True:
            linha = self.rfile.readline()
            if not linha:
                return
            c = linha.decode("utf-8", "replace").strip()
            alto = c.upper()
            if alto.startswith(("EHLO", "HELO")):
                self.wfile.write(b"250 rele-falso\r\n")
            elif alto.startswith("MAIL FROM"):
                de = c.split(":", 1)[1].strip()
                self.wfile.write(b"250 OK\r\n")
            elif alto.startswith("RCPT TO"):
                para.append(c.split(":", 1)[1].strip())
                self.wfile.write(b"250 OK\r\n")
            elif alto == "DATA":
                self.wfile.write(b"354 manda o texto, termina com .\r\n")
                partes = []
                while True:
                    l = self.rfile.readline()
                    if not l or l.strip() == b".":
                        break
                    partes.append(l.decode("utf-8", "replace"))
                corpo = "".join(partes)
                with CAIXA_TRAVA:
                    CAIXA.append({"de": de, "para": para, "bruto": corpo})
                self.wfile.write(b"250 OK: fila 0001\r\n")
                de, para, corpo = "", [], ""
            elif alto == "QUIT":
                self.wfile.write(b"221 tchau\r\n")
                return
            elif alto == "RSET":
                de, para, corpo = "", [], ""
                self.wfile.write(b"250 OK\r\n")
            else:
                self.wfile.write(b"250 OK\r\n")

    def handle_error(self, *_):
        pass


class ReleTcp(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def handle_error(self, *_):
        pass


def assunto_e_corpo(msg):
    """Desmonta a mensagem RFC 5322 que o `email.rs` monta."""
    cabecalho, _, corpo = msg["bruto"].partition("\r\n\r\n")
    assunto = ""
    for l in cabecalho.split("\r\n"):
        if l.lower().startswith("subject:"):
            assunto = l.split(":", 1)[1].strip()
            if assunto.startswith("=?UTF-8?B?"):
                assunto = base64.b64decode(
                    assunto[len("=?UTF-8?B?") : -2]
                ).decode("utf-8", "replace")
    texto = base64.b64decode(corpo.replace("\r\n", "")).decode("utf-8", "replace")
    return assunto, texto


def caixa():
    with CAIXA_TRAVA:
        return list(CAIXA)


def esperar_email(quantos, segundos=8.0):
    """O envio de job roda em linha de execucao propria: espera, nao adivinha."""
    fim = time.time() + segundos
    while time.time() < fim:
        if len(caixa()) >= quantos:
            return True
        time.sleep(0.1)
    return len(caixa()) >= quantos


# ------------------------------------------------------------------ servidor
class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer."""

    def __init__(self, nome, porta, avisar_seguranca=True):
        self.nome, self.porta = nome, porta
        self.dir = f"{BASE}/{nome}"
        self.avisar_seguranca = avisar_seguranca
        self.s = self.f = None

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        os.makedirs(self.dir + "/dados", exist_ok=True)
        cfg = {
            "bind": f"127.0.0.1:{self.porta}",
            "base": self.dir + "/dados",
            "token": "t",
            "log_acessos": self.dir + "/acessos.log",
            "web": {"ligado": False},
            "seguranca": {
                "comandos_proibidos": PROIBIDOS,
                "bases_proibidas": BASES_PROIBIDAS,
                "bloqueio_minutos": 60,
                "blacklist": self.dir + "/blacklist.json",
                # O jobs.json aponta para a base da bancada: sem isto o servidor grava
                # `jobs.json`/`jobs.log` no cwd -- e o cwd era a RAIZ do repositorio.
                "jobs": self.dir + "/jobs.json",
            },
            "alertas": {
                "ligado": False,
                "email": {
                    "ligado": True,
                    "avisar_jobs": True,
                    "avisar_seguranca": self.avisar_seguranca,
                    "servidor": "127.0.0.1",
                    "porta": PORTA_SMTP,
                    "de": "phxsql@bancada",
                    "para": ["admin@bancada"],
                    "timeout_s": 5,
                },
            },
        }
        self.cfg = self.dir + "/config.json"
        with open(self.cfg, "w") as f:
            json.dump(cfg, f, indent=2)
        self.saida = open(self.dir + "/servidor.err", "wb")
        self.p = subprocess.Popen(
            [BINARIO, "--config", self.cfg],
            stdout=self.saida,
            stderr=subprocess.STDOUT,
        )
        for _ in range(120):
            try:
                socket.create_connection(("127.0.0.1", self.porta), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor {self.nome} nao subiu na porta {self.porta}")
        self.abrir()
        return self

    def abrir(self):
        self.fechar()
        self.s = socket.create_connection(("127.0.0.1", self.porta), 5)
        self.f = self.s.makefile("rwb")

    def fechar(self):
        for x in (self.f, self.s):
            try:
                if x:
                    x.close()
            except OSError:
                pass
        self.f = self.s = None

    def __exit__(self, *_):
        self.fechar()
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        self.saida.close()

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r

    def erros(self):
        with open(self.dir + "/servidor.err") as f:
            return f.read()

    def blacklist(self):
        c = self.dir + "/blacklist.json"
        if not os.path.exists(c):
            return {}
        with open(c) as f:
            return json.load(f)


def diz(rotulo, ok, detalhe=""):
    # O detalhe so aparece na FALHA: um "OK" seguido do texto que explicaria a
    # falha e a mentira mais facil de deixar numa saida colada em documento.
    print(f"  [{'OK  ' if ok else 'FALHA'}] {rotulo}" + (f" -- {detalhe}" if detalhe and not ok else ""))
    return ok


RESULTADO = {"quando_utc": time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())}
FALHAS = []


def afirmar(rotulo, ok, detalhe=""):
    if not diz(rotulo, ok, detalhe):
        FALHAS.append(rotulo)
    return ok


# ---------------------------------------------------------------- as partes
def parte_0_o_instrumento(sv):
    print("\n=== 0. O INSTRUMENTO antes do veredito: o SMTP falso pega e-mail do motor?\n")
    r = sv.pedir(
        op="job_salvar",
        nome="quebra",
        descricao="job que falha de proposito, para provar o rele",
        ligado=False,
        agenda={"tipo": "manual"},
        pedido={"op": "varrer", "database": "naoexiste", "tabela": "naoexiste"},
    )
    afirmar("job_salvar 'quebra' gravado", r.get("ok"), str(r.get("erro", ""))[:120])
    r = sv.pedir(op="job_rodar", nome="quebra")
    print("    resposta job_rodar:", json.dumps(r, ensure_ascii=False)[:200])
    chegou = esperar_email(1)
    afirmar("o SMTP falso recebeu o e-mail de job que falhou", chegou,
            f"{len(caixa())} mensagem(ns) na caixa")
    if chegou:
        a, c = assunto_e_corpo(caixa()[0])
        print(f"    assunto: {a}")
        print("    corpo (2 primeiras linhas):")
        for l in c.splitlines()[:2]:
            print(f"      {l}")
        RESULTADO["controle_instrumento_assunto"] = a
    RESULTADO["controle_instrumento"] = bool(chegou)
    return len(caixa())


def parte_1_controle_positivo(sv, base_emails):
    print("\n=== 1. CONTROLE POSITIVO: comando PERMITIDO nao bloqueia nem manda e-mail\n")
    r = sv.pedir(op="criar_database", database="x")
    afirmar("criar_database x (permitido) passou", r.get("ok"), str(r.get("erro", ""))[:120])
    r = sv.pedir(op="bancos")
    afirmar("bancos (permitido) passou", r.get("ok"), str(r.get("erro", ""))[:120])
    r = sv.pedir(
        op="criar_tabela", database="x", tabela="folha",
        colunas=[{"nome": "id", "tipo": "Int8"}, {"nome": "nome", "tipo": "Str(40)"}],
        indices=[{"nome": "porId", "colunas": ["id"], "unico": True}],
    )
    afirmar("criar_tabela x.folha (permitido) passou", r.get("ok"), str(r.get("erro", ""))[:120])
    time.sleep(1.0)
    novos = len(caixa()) - base_emails
    afirmar("nenhum e-mail novo depois dos permitidos", novos == 0, f"{novos} novo(s)")
    b = sv.pedir(op="bloqueios")
    ativos = b.get("bloqueios") or b.get("ativos") or []
    afirmar("nenhum IP bloqueado depois dos permitidos", len(ativos) == 0, json.dumps(b)[:160])
    RESULTADO["permitido_emails"] = novos
    RESULTADO["permitido_bloqueios"] = len(ativos)
    return len(caixa())


def parte_2_comando_proibido(sv, base_emails):
    print("\n=== 2. O COMANDO PROIBIDO: recusa, codigo, bloqueio -- e e-mail?\n")
    r = sv.pedir(op="excluir_tabela", database="x", tabela="folha", confirmar="folha")
    erro = str(r.get("erro", ""))
    print(f"    resposta: {json.dumps(r, ensure_ascii=False)[:300]}")
    afirmar("excluir_tabela (proibido) foi RECUSADO", not r.get("ok"), erro[:120])
    afirmar("o codigo e SP000025 (ACESSO_NEGADO)", "SP000025" in erro, erro[:120])
    afirmar("a recusa diz que o IP foi bloqueado", "bloque" in erro.lower(), erro[:160])
    RESULTADO["recusa_comando"] = erro

    bl = sv.blacklist()
    lista = bl.get("bloqueios", []) if isinstance(bl, dict) else []
    afirmar("o IP 127.0.0.1 entrou na blacklist em disco", any(
        b.get("ip") == "127.0.0.1" for b in lista), json.dumps(bl)[:200])
    if lista:
        print("    blacklist.json:", json.dumps(lista[0], ensure_ascii=False))
        RESULTADO["bloqueio"] = lista[0]

    time.sleep(1.5)
    novos = len(caixa()) - base_emails
    RESULTADO["emails_da_violacao"] = novos
    print(f"\n    >>> e-mails novos depois do comando proibido: {novos}")
    for m in caixa()[base_emails:]:
        a, c = assunto_e_corpo(m)
        print(f"    assunto: {a}")
        for l in c.splitlines():
            print(f"      {l}")
    afirmar("o administrador foi avisado por e-mail da violacao grave", novos >= 1,
            "NENHUM e-mail saiu -- ver a secao 'O que NAO existe'")

    print("\n    o que o servidor gravou no proprio erro padrao:")
    for l in sv.erros().splitlines():
        if "BLOQUEA" in l.upper() or "AVISO" in l.upper():
            print(f"      {l}")
    return len(caixa())


def parte_3_ip_barrado(sv):
    print("\n=== 3. Depois do bloqueio, a MESMA maquina nao entra mais\n")
    sv.abrir()
    r = sv.pedir(op="bancos")
    erro = str(r.get("erro", ""))
    afirmar("uma conexao nova do IP bloqueado e recusada", not r.get("ok"), erro[:160])
    print(f"    resposta: {json.dumps(r, ensure_ascii=False)[:260]}")
    RESULTADO["recusa_pos_bloqueio"] = erro


def parte_4_base_proibida(sv, base_emails):
    print("\n=== 4. A BASE proibida, no segundo servidor (o primeiro se auto-bloqueou)\n")
    r = sv.pedir(op="bancos")
    afirmar("controle: bancos passa neste servidor", r.get("ok"), str(r.get("erro", ""))[:120])
    r = sv.pedir(op="varrer", database="financeiro", tabela="qualquer")
    erro = str(r.get("erro", ""))
    print(f"    resposta: {json.dumps(r, ensure_ascii=False)[:300]}")
    afirmar("varrer na base proibida foi RECUSADO", not r.get("ok"), erro[:120])
    afirmar("o codigo e SP000025 (ACESSO_NEGADO)", "SP000025" in erro, erro[:120])
    bl = sv.blacklist()
    lista = bl.get("bloqueios", []) if isinstance(bl, dict) else []
    afirmar("o IP entrou na blacklist tambem pela base proibida", any(
        b.get("ip") == "127.0.0.1" for b in lista), json.dumps(bl)[:200])
    time.sleep(1.5)
    novos = len(caixa()) - base_emails
    RESULTADO["emails_da_base_proibida"] = novos
    RESULTADO["recusa_base"] = erro
    print(f"\n    >>> e-mails novos depois da base proibida: {novos}")
    for m in caixa()[base_emails:]:
        a, c = assunto_e_corpo(m)
        print(f"    assunto: {a}")
        for l in c.splitlines():
            print(f"      {l}")
    afirmar("o administrador foi avisado por e-mail da base proibida", novos >= 1,
            "NENHUM e-mail saiu -- ver a secao 'O que NAO existe'")
    return len(caixa())


def parte_5_guarda_pedida(sv, base_emails):
    print("\n=== 5. GUARDA PEDIDA: sem avisar_seguranca o bloqueio acontece CALADO\n")
    r = sv.pedir(op="reindexar", database="x", tabela="y")
    erro = str(r.get("erro", ""))
    afirmar("reindexar (proibido) foi recusado tambem aqui", not r.get("ok"), erro[:120])
    bl = sv.blacklist()
    lista = bl.get("bloqueios", []) if isinstance(bl, dict) else []
    afirmar("o IP foi bloqueado do mesmo jeito", any(
        b.get("ip") == "127.0.0.1" for b in lista), json.dumps(bl)[:160])
    time.sleep(1.5)
    novos = len(caixa()) - base_emails
    RESULTADO["emails_sem_avisar_seguranca"] = novos
    afirmar("nenhum e-mail saiu, porque ninguem pediu", novos == 0, f"{novos} novo(s)")


def main():
    print(f"# Bateria dos comandos proibidos -- {RESULTADO['quando_utc']} UTC")
    commit = subprocess.run(
        ["git", "-C", RAIZ, "rev-parse", "--short", "HEAD"],
        capture_output=True, text=True).stdout.strip()
    versao = subprocess.run([BINARIO, "-V"], capture_output=True, text=True)
    RESULTADO["commit"] = commit
    RESULTADO["versao"] = (versao.stdout or versao.stderr).strip()
    print(f"# commit {commit} -- {RESULTADO['versao']}")
    print(f"# comandos_proibidos={PROIBIDOS} bases_proibidas={BASES_PROIBIDAS}")
    print(f"# portas: dados {PORTA_A}, {PORTA_B} e {PORTA_C}; SMTP falso {PORTA_SMTP}")

    rele = ReleTcp(("127.0.0.1", PORTA_SMTP), Rele)
    threading.Thread(target=rele.serve_forever, daemon=True).start()
    try:
        with Servidor("a", PORTA_A) as sa:
            n = parte_0_o_instrumento(sa)
            n = parte_1_controle_positivo(sa, n)
            n = parte_2_comando_proibido(sa, n)
            parte_3_ip_barrado(sa)
        with Servidor("b", PORTA_B) as sb:
            n = parte_4_base_proibida(sb, n)
        with Servidor("c", PORTA_C, avisar_seguranca=False) as sc:
            parte_5_guarda_pedida(sc, n)
    finally:
        rele.shutdown()
        rele.server_close()
        shutil.rmtree(BASE, ignore_errors=True)

    RESULTADO["emails_no_total"] = len(caixa())
    RESULTADO["falhas"] = FALHAS
    with open(os.path.join(RAIZ, "bancada/proibidos/resultados.json"), "w") as f:
        json.dump(RESULTADO, f, indent=2, ensure_ascii=False)
    print(f"\n=== {len(FALHAS)} afirmacao(oes) falharam: {FALHAS}")
    print("=== resultados em bancada/proibidos/resultados.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
