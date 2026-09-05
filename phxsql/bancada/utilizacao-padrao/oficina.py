#!/usr/bin/env python3
"""O que as duas bancadas de utilizacao padrao compartilham.

Nao ha medicao aqui: so subir o servidor, falar com ele CONTANDO OS BYTES, e
somar o que cada arquivo da tabela ocupa no disco. As duas medicoes moram nos
scripts ao lado.

# Por que a conexao conta bytes

Porque «20.000 linhas com blob» e «20.000 linhas sem blob» nao sao o mesmo
trabalho, e a diferenca entre elas passa por tres lugares ao mesmo tempo: o
tamanho no fio, o `.bin`/`.memo` e o slot do `.reg`. Publicar so o tempo
juntaria os tres num numero so -- que e a armadilha que a `bancada/LEIA-ME.md`
descreve duas vezes. Contar os bytes que entram e saem do soquete separa o
primeiro dos outros dois.
"""
import json
import os
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))

# Reuso, e nao quarta copia: o binario, o token e o hash da senha ja estao
# escritos na bancada do profiler.
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))
from comum import PHXSQLD, TOKEN, hash_da_senha, baixar  # noqa: E402,F401

USUARIO = "adm"
SENHA = "senha-do-adm"

# As oito extensoes que uma tabela pode ter no disco, na ordem em que o
# `esquema` as devolve. A lista sai daqui e nao de um `glob` solto porque o
# relatorio precisa dizer ZERO para o arquivo que nao existe -- «nao apareceu
# na listagem» e «tem zero byte» sao coisas diferentes, e so a segunda e
# resposta.
EXTENSOES = ["reg", "ndx", "bin", "memo", "log", "trash", "reason", "pag"]


def config(porta, max_linhas=25_000):
    """O `config.json` de FABRICA, mais o teto de linhas que a leitura de volta
    precisa. Nada de `recursos`: o que se mede aqui e o servidor como ele sai
    da caixa, e a janela de durabilidade padrao faz parte disso."""
    return {
        "base": "base",
        "bind": "127.0.0.1:%d" % porta,
        "token": TOKEN,
        "max_linhas": max_linhas,
        "web": {"ligado": False},
        "usuarios": [
            {"login": USUARIO, "nome": "Adriano", "id": 10, "nivel": "admin",
             "senha_hash": hash_da_senha(SENHA),
             "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                             "excluir": True, "criar": True,
                             "administrar": True, "verificar": True,
                             "reindexar": True}}},
        ],
    }


def subir(base, porta, cfg=None):
    os.makedirs(base, exist_ok=True)
    with open(os.path.join(base, "config.json"), "w") as f:
        json.dump(cfg or config(porta), f, indent=2)
    log = open(os.path.join(base, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=base, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(160):
        time.sleep(0.1)
        try:
            socket.create_connection(("127.0.0.1", porta), 0.3).close()
            return p
        except OSError:
            if p.poll() is not None:
                raise SystemExit("o servidor morreu ao subir -- veja %s/servidor.log" % base)
    p.kill()
    raise SystemExit("o servidor nao subiu na porta %d" % porta)


class Conexao:
    """Uma conexao que CONTA os bytes de cada sentido.

    O `TCP_NODELAY` esta aqui pelo motivo medido em
    `cognicao_soquete-sem-tcp-nodelay-mede-o-nagle`: sem ele, Nagle mais o ACK
    adiado do outro lado poem ~40 ms parados em cada ida e volta, e o que se
    mediria seria o soquete, nao o servidor."""

    def __init__(self, porta, prazo=120):
        self.s = socket.create_connection(("127.0.0.1", porta))
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.s.settimeout(prazo)
        self.f = self.s.makefile("rwb")
        self.enviados = 0
        self.recebidos = 0
        # O `ms` que o PROPRIO servidor carimba em cada resposta. Somar isto ao
        # lado do tempo de parede separa o que aconteceu dentro do motor do que
        # aconteceu no cliente e no fio -- e essa separacao ja pagou por si: a
        # primeira corrida de cada lado saiu 1,9x mais lenta que a segunda, e
        # sem este contador o numero teria virado «blob custa 1,9x».
        self.ms = 0.0
        r = self.fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
        if not r.get("ok"):
            raise SystemExit("login: %s" % r)

    def zerar(self):
        self.enviados = 0
        self.recebidos = 0
        self.ms = 0.0

    def fala(self, p):
        p.setdefault("token", TOKEN)
        linha = (json.dumps(p) + "\n").encode()
        self.enviados += len(linha)
        self.f.write(linha)
        self.f.flush()
        volta = self.f.readline()
        self.recebidos += len(volta)
        r = json.loads(volta.decode())
        self.ms += r.get("ms") or 0
        return r

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p.get("op"), r.get("erro")))
        return r["resultado"]

    def erro(self, p):
        """O contrario do `ok`: exige que o servidor RECUSE, e devolve o texto
        da recusa. Existe porque metade das provas desta bancada e sobre o que
        o motor NAO deixa fazer."""
        r = self.fala(p)
        if r.get("ok"):
            raise SystemExit("%s: devia ter recusado e passou -- %s"
                             % (p.get("op"), json.dumps(r)[:300]))
        return r.get("erro", "")

    def fechar(self):
        for c in (self.f, self.s):
            try:
                c.close()
            except OSError:
                pass


def bytes_no_disco(base, database, tabela):
    """Quantos bytes cada arquivo da tabela ocupa, por extensao.

    Soma TODOS os volumes: uma tabela paginada tem `nome_001.reg`,
    `nome_002.reg`… e uma alfanumerica tem `nome_A.reg`, `nome_B.reg`… O que
    interessa e o que a tabela ocupa, e nao o que o primeiro arquivo dela
    ocupa."""
    pasta = os.path.join(base, "base", database)
    saida = {e: 0 for e in EXTENSOES}
    if not os.path.isdir(pasta):
        return saida
    for nome in os.listdir(pasta):
        if "." not in nome:
            continue
        raiz, ext = nome.rsplit(".", 1)
        if ext not in saida:
            continue
        # `pedidos`, `pedidos_001` e `pedidos_A` sao a mesma tabela; `pedidos2`
        # nao e. O corte no sublinhado e o que separa os dois casos.
        if raiz != tabela and not raiz.startswith(tabela + "_"):
            continue
        saida[ext] += os.path.getsize(os.path.join(pasta, nome))
    return saida


def portao_de_medicao():
    """O `bancada/esta-medindo.sh`: sai 0 quando ACHOU medicao em curso.

    Devolve `True` quando a maquina esta ocupada -- e ai nenhum numero de TEMPO
    desta bancada se publica. As contagens (bytes, `fsync`, linhas conferidas)
    continuam valendo: elas nao dependem de quem mais esta rodando."""
    r = subprocess.run(["bash", os.path.join(RAIZ, "bancada", "esta-medindo.sh")],
                       capture_output=True, text=True)
    return r.returncode == 0, r.stdout.strip()
