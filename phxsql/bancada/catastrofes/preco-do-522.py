#!/usr/bin/env python3
"""O PRECO do pedido 522, contra o `phxsqld` de pe.

Desde o 522 o `fechar` nao baixa mais o byte 52 do `.ndx` -- so o fecho da
janela, depois dos `fsync`. Um processo que cai deixa marcada TODA tabela
escrita desde o ultimo fecho da janela, e o arranque seguinte as reconstroi
(`Database::reconstruir_indices_marcados`). Este roteiro mede as duas coisas
que o preco quer dizer, num banco de exemplo:

  * QUANTAS tabelas ficam marcadas depois de um SIGKILL sob carga, com a janela
    de fabrica (200 operacoes ou 200 ms);
  * QUANTO o arranque leva para reconstrui-las -- a linha `tempo` do relatorio
    de recuperacao, que o proprio servidor imprime.

E o CONTROLE, na mesma rodada: o SIGKILL depois de a janela fechar sozinha (o
relogio de fundo, servidor ocioso) nao deixa tabela marcada nenhuma, e o
arranque nao reconstroi nada. Sem ele, «marcou N» podia ser o instrumento
contando tabela limpa.

    python3 bancada/catastrofes/preco-do-522.py --binario target/release/phxsqld
    python3 bancada/catastrofes/preco-do-522.py --tabelas 8 --linhas 20000 --segundos 3 --rodadas 3

Nenhum numero e digitado: o JSON sai do byte 52 lido do arquivo e do relatorio
do servidor. Mata so o PID que subiu -- nunca pkill.
"""
import argparse
import datetime
import json
import os
import platform
import re
import shutil
import signal
import socket
import subprocess
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
TOKEN = "token-de-servico"
DB = "preco"


def porta_livre():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def hash_da_senha(binario, senha):
    saida = subprocess.run([binario, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def config(binario, porta):
    return {
        # bancada de teste, NAO cliente do produto: fala em claro para medir o
        # arranque sem o aperto de mao no meio.
        "base": "base", "bind": "127.0.0.1:%d" % porta,
        "cifra_fio": {"exigir": False}, "token": TOKEN,
        "web": {"ligado": False},
        # A janela de FABRICA, dita por extenso para o resultado nao mudar se
        # o padrao mudar: 200 operacoes ou 200 ms.
        "recursos": {"durabilidade": "por_lote", "lote_operacoes": 200,
                     "lote_milissegundos": 200},
        "usuarios": [{
            "login": "adm", "nome": "Adriano", "id": 10, "nivel": "admin",
            "senha_hash": hash_da_senha(binario, "senha-do-adm"),
            "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                            "excluir": True, "criar": True,
                            "administrar": True, "verificar": True}},
        }],
    }


def subir(binario, base, porta):
    # Um log por arranque: o relatorio lido e o DESTE arranque, nunca o de
    # um anterior.
    log = open(os.path.join(base, "servidor.log"), "w")
    comeco = time.monotonic()
    p = subprocess.Popen([binario], cwd=base, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(1200):
        try:
            socket.create_connection(("127.0.0.1", porta), 0.3).close()
            return p, time.monotonic() - comeco
        except OSError:
            if p.poll() is not None:
                raise SystemExit("o servidor morreu ao subir -- veja %s/servidor.log" % base)
            time.sleep(0.05)
    p.kill()
    raise SystemExit("o servidor nao subiu na porta %d" % porta)


def matar(p):
    """SIGKILL no PID que ESTE roteiro subiu: e a queda que se quer medir."""
    if p.poll() is None:
        p.send_signal(signal.SIGKILL)
        p.wait(10)


def parar(p):
    if p.poll() is None:
        p.send_signal(signal.SIGTERM)
        try:
            p.wait(10)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait(5)


class Ligacao:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta))
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.s.settimeout(60)
        self.f = self.s.makefile("rwb")
        r = self.fala({"op": "login", "usuario": "adm", "senha": "senha-do-adm"})
        if not r.get("ok"):
            raise SystemExit("login: %s" % r)

    def manda(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()

    def fala(self, p):
        self.manda(p)
        return json.loads(self.f.readline().decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p.get("op"), r.get("erro")))
        return r["resultado"]

    def fechar(self):
        # Os DOIS: `makefile()` segura o descritor por baixo.
        for c in (self.f, self.s):
            try:
                c.close()
            except OSError:
                pass


def nome(i):
    return "t%02d" % i


def byte_52(base, tabela):
    with open(os.path.join(base, "base", DB, tabela + ".ndx"), "rb") as f:
        return f.read(53)[52]


def relatorio(base):
    """A recuperacao do ULTIMO arranque: (indices reconstruidos, tempo ms)."""
    texto = open(os.path.join(base, "servidor.log"), encoding="utf-8",
                 errors="replace").read()
    blocos = texto.split("PHXSQL Recovery")
    if len(blocos) < 2:
        return 0, None
    ultimo = blocos[-1]
    rec = re.search(r"indices reconstruidos \.+ (\d+)", ultimo)
    ms = re.search(r"tempo \.+ (\d+) ms", ultimo)
    return (int(rec.group(1)) if rec else 0), (int(ms.group(1)) if ms else None)


def rodada(binario, tabelas, linhas, segundos):
    base = tempfile.mkdtemp(prefix="phx-preco-522-")
    try:
        porta = porta_livre()
        with open(os.path.join(base, "config.json"), "w") as f:
            json.dump(config(binario, porta), f, indent=2)
        p, _ = subir(binario, base, porta)
        c = Ligacao(porta)
        c.ok({"op": "criar_database", "database": DB})
        for i in range(tabelas):
            c.ok({"op": "criar_tabela", "database": DB, "tabela": nome(i),
                  "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                              {"nome": "cidade", "tipo": "Str(20)"}],
                  "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                               "primario": True},
                              {"nome": "porCidade", "colunas": ["cidade"]}]})
            for lote in range(0, linhas, 1000):
                c.ok({"op": "inserir_lote", "database": DB, "tabela": nome(i),
                      "linhas": [{"id": k, "cidade": "c%d" % (k % 7)}
                                 for k in range(lote + 1, min(lote + 1000, linhas) + 1)]})

        # CONTROLE: ocioso por mais que a janela -- o relogio de fundo fecha e
        # o SIGKILL nao deixa marca nenhuma.
        time.sleep(1.0)
        c.fechar()
        matar(p)
        controle_marcadas = sum(byte_52(base, nome(i)) for i in range(tabelas))
        # O arranque sem marca nenhuma paga so a varredura dos cabecalhos: e
        # o custo que o passe acrescenta a TODO arranque.
        p, controle_arranque_s = subir(binario, base, porta)
        controle_reconstruidas, controle_ms = relatorio(base)

        # A QUEDA SOB CARGA: insercoes uma a uma, em rodizio pelas tabelas, e
        # o SIGKILL com um pedido no fio.
        c = Ligacao(porta)
        prox = linhas + 1
        fim = time.monotonic() + segundos
        pedidos = 0
        while time.monotonic() < fim:
            for i in range(tabelas):
                c.ok({"op": "inserir", "database": DB, "tabela": nome(i),
                      "linha": {"id": prox, "cidade": "carga"}})
                pedidos += 1
            prox += 1
        c.manda({"op": "inserir", "database": DB, "tabela": nome(0),
                 "linha": {"id": prox, "cidade": "no-fio"}})
        matar(p)
        c.fechar()
        marcadas = sum(byte_52(base, nome(i)) for i in range(tabelas))

        p, arranque_s = subir(binario, base, porta)
        reconstruidas, ms = relatorio(base)
        # Depois do arranque toda tabela responde pela chave -- nenhuma manda
        # «reparar indice».
        c = Ligacao(porta)
        respondem = 0
        for i in range(tabelas):
            r = c.fala({"op": "buscar", "database": DB, "tabela": nome(i),
                        "indice": "pk", "chave": [1]})
            respondem += 1 if r.get("ok") else 0
        c.fechar()
        parar(p)
        return {
            "tabelas": tabelas, "linhas_por_tabela": linhas,
            "pedidos_na_carga": pedidos,
            "marcadas_depois_da_queda": marcadas,
            "reconstruidas_no_arranque": reconstruidas,
            "tempo_da_recuperacao_ms": ms,
            "arranque_ate_a_porta_s": round(arranque_s, 3),
            "respondem_depois": respondem,
            "controle_marcadas_ocioso": controle_marcadas,
            "controle_reconstruidas": controle_reconstruidas,
            "controle_tempo_ms": controle_ms,
            "controle_arranque_ate_a_porta_s": round(controle_arranque_s, 3),
        }
    finally:
        shutil.rmtree(base, ignore_errors=True)


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--binario", default=os.path.join(RAIZ, "target", "release", "phxsqld"))
    a.add_argument("--tabelas", type=int, default=8)
    a.add_argument("--linhas", type=int, default=20000)
    a.add_argument("--segundos", type=float, default=3.0)
    a.add_argument("--rodadas", type=int, default=3)
    a.add_argument("--saida", default=os.path.join(AQUI, "preco-do-522.json"))
    x = a.parse_args()
    # Absoluto: o servidor sobe com `cwd` na base da rodada.
    x.binario = os.path.abspath(x.binario)
    if not os.path.exists(x.binario):
        raise SystemExit("NAO MEDIDO: falta o binario %s (cargo build --release -p phxsql-server --bin phxsqld)" % x.binario)
    rodadas = []
    for r in range(x.rodadas):
        m = rodada(x.binario, x.tabelas, x.linhas, x.segundos)
        rodadas.append(m)
        print("rodada %d: %s" % (r + 1, json.dumps(m)))
    json.dump({
        "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "nucleo": platform.release(),
        "binario": os.path.relpath(x.binario, RAIZ) if x.binario.startswith(RAIZ) else os.path.basename(x.binario),
        "rodadas": rodadas,
    }, open(x.saida, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print("gravado: %s" % x.saida)


if __name__ == "__main__":
    main()
