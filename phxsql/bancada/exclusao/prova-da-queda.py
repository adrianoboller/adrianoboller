#!/usr/bin/env python3
"""A PROVA da janela de durabilidade da exclusão: matar o servidor no meio dela.

    cargo build --release
    python3 bancada/exclusao/prova-da-queda.py

# O que se prova aqui, e por que teste unitário não bastava

`recursos.exclusao_na_janela` tira o `fsync` de dentro de cada exclusão física.
Isso abre um intervalo entre o `excluir` responder OK e o `.trash` estar no
disco, e a pergunta que o sprint mandou responder antes de uma linha de código
é **o que exatamente se perde numa queda dentro desse intervalo**.

A casa já escreveu a lição: *teste unitário não prova queda de conexão —
soquete prova*. Aqui é a mesma família. Um `#[test]` que fecha a `Table` sem
sincronizar prova alguma coisa, mas quem fecha a `Table` executa `Drop`,
libera descritores e volta ao teste — nada disso é uma queda. Este script mata
um `phxsqld` **de verdade** com `SIGKILL`, que é o que um `kill -9`, um OOM ou
um `panic` fazem: o processo some sem executar mais uma instrução.

O que a queda do PROCESSO **não** leva: o `write` já foi entregue ao sistema
operacional, e quem reabre o arquivo lê a mesma página. Por isso a expectativa,
nos dois modos, é a mesma — e a prova é essa igualdade.

O que a queda de ENERGIA leva está em `docs/DESEMPENHO.md` §7, e não se prova
com script nenhum: nenhum processo em espaço de usuário consegue provocar uma
queda de energia.

# A conferência que o sprint exige

Para cada linha excluída, depois da queda, uma das três:

    no `.reg` e não no `.trash`   a exclusão não aconteceu — nada se perde
    no `.trash` e não no `.reg`   a exclusão aconteceu e é reversível
    nos dois                      duplicada — o lado que a casa escolheu

E a quarta, **em nenhum dos dois**, é a que mata o sprint. É ela que este
script procura, linha a linha, e é ela que ele imprime se achar.

# Portas e processos

Sobe um `phxsqld` próprio em **7100** (dados). Mata **só** os PIDs que ele
mesmo criou — nunca `pkill`.
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))

from comum import PHXSQLD, TOKEN, Conexao, hash_da_senha  # noqa: E402

PORTA = 7100
BASE = os.path.join(RAIZ, "bancada", "exclusao", ".base-da-prova")
DB = "quedas"
TAB = "pedidos"
LINHAS = 400
EXCLUIR = 150


def config(na_janela):
    """A janela mais larga que a configuração permite, de propósito.

    `lote_operacoes` e `lote_milissegundos` altos deixam a janela ABERTA
    durante a corrida inteira: é o pior caso da garantia, e é nele que a prova
    tem valor. Uma janela de 200 ms fecharia sozinha antes do `kill`, e o
    script estaria provando que o relógio funciona.
    """
    return {
        "base": "base",
        "bind": "127.0.0.1:%d" % PORTA,
        "token": TOKEN,
        "web": {"ligado": False},
        "recursos": {
            "exclusao_na_janela": na_janela,
            "lote_operacoes": 1_000_000,
            "lote_milissegundos": 3_600_000,
        },
        "usuarios": [
            {"login": "adm", "nome": "Adriano", "id": 10, "nivel": "admin",
             "senha_hash": hash_da_senha("senha-do-adm"),
             "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                             "excluir": True, "criar": True,
                             "administrar": True, "verificar": True}}},
        ],
    }


def subir(na_janela, limpar):
    if limpar:
        shutil.rmtree(BASE, ignore_errors=True)
    os.makedirs(BASE, exist_ok=True)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump(config(na_janela), f, indent=2)
    saida = open(os.path.join(BASE, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=saida,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(80):
        time.sleep(0.25)
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.4).close()
            return p
        except OSError:
            continue
    p.kill()
    raise SystemExit("o servidor nao subiu na porta %d" % PORTA)


def matar_de_verdade(p):
    """SIGKILL, e não SIGTERM.

    O `phxsqld` trata o SIGTERM: ele fecha a janela, sincroniza e sai limpo —
    que é o contrário do que este script quer. `SIGKILL` não é entregue ao
    programa; o núcleo derruba o processo onde ele estiver.
    """
    os.kill(p.pid, signal.SIGKILL)
    p.wait(10)


def preparar(c):
    c.entrar("adm", "senha-do-adm")
    c.ok({"op": "criar_database", "database": DB})
    c.ok({"op": "criar_tabela", "database": DB, "tabela": TAB,
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "cliente", "tipo": "Str(40)"},
                      {"nome": "obs", "tipo": "Memo"}],
          "indices": [{"nome": "porId", "colunas": ["id"],
                       "unico": True, "primaria": True}]})
    for i in range(1, LINHAS + 1):
        c.ok({"op": "inserir", "database": DB, "tabela": TAB,
              "valores": {"id": i, "cliente": "Cliente %04d" % i,
                          "obs": "observacao razoavelmente longa da linha %d" % i}})


def rodada(na_janela, rotulo):
    print("\n=== %s (exclusao_na_janela=%s) ===" % (rotulo, na_janela))
    p = subir(na_janela, limpar=True)
    try:
        c = Conexao(PORTA)
        preparar(c)
        # Os alvos espalhados pela tabela inteira, como a bancada faz.
        alvos = sorted({(k * 7919) % LINHAS + 1 for k in range(EXCLUIR)})
        respondidas = []
        for rowid in alvos:
            r = c.ok({"op": "excluir", "database": DB, "tabela": TAB,
                      "rowid": rowid, "fisico": True})
            if r["excluido"]:
                respondidas.append(rowid)
        print("  %d exclusoes responderam OK" % len(respondidas))
        c.fechar()
        matar_de_verdade(p)
        p = None
        print("  SIGKILL entregue: o processo morreu sem sincronizar nada")
    finally:
        if p is not None:
            p.kill()
            p.wait(5)

    # Reabre e confere, linha a linha.
    p = subir(na_janela, limpar=False)
    try:
        c = Conexao(PORTA)
        c.entrar("adm", "senha-do-adm")
        lixo = c.ok({"op": "lixeira", "database": DB, "tabela": TAB,
                     "limite": 0})
        no_trash = {d["rowid"] for d in lixo["descartadas"]}
        no_reg = set()
        # `ler` devolve a linha, ou nulo -- e nulo chega aqui como `None`.
        for rowid in respondidas:
            if c.ok({"op": "ler", "database": DB, "tabela": TAB,
                     "rowid": rowid}) is not None:
                no_reg.add(rowid)
        # E as que ninguem mandou excluir continuam la?
        vivas = [r for r in range(1, LINHAS + 1) if r not in respondidas]
        sumiram_sem_pedido = [
            r for r in vivas[:60]
            if c.ok({"op": "ler", "database": DB, "tabela": TAB,
                     "rowid": r}) is None
        ]
        c.fechar()
    finally:
        p.send_signal(signal.SIGTERM)
        try:
            p.wait(10)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait(5)

    so_no_reg = [r for r in respondidas if r in no_reg and r not in no_trash]
    so_no_trash = [r for r in respondidas if r in no_trash and r not in no_reg]
    nos_dois = [r for r in respondidas if r in no_trash and r in no_reg]
    em_nenhum = [r for r in respondidas if r not in no_trash and r not in no_reg]

    print("  so no .reg   (a exclusao nao aconteceu) : %d" % len(so_no_reg))
    print("  so no .trash (aconteceu, e reversivel)  : %d" % len(so_no_trash))
    print("  nos dois     (duplicada)                : %d" % len(nos_dois))
    print("  EM NENHUM    (o caso que mata o sprint) : %d" % len(em_nenhum))
    if em_nenhum:
        print("  os rowids: %s" % em_nenhum[:20])
    if sumiram_sem_pedido:
        print("  LINHAS QUE NINGUEM MANDOU EXCLUIR SUMIRAM: %s"
              % sumiram_sem_pedido[:20])
    return len(em_nenhum) == 0 and not sumiram_sem_pedido


def main():
    if not os.path.exists(PHXSQLD):
        raise SystemExit("falta %s -- rode `cargo build --release`" % PHXSQLD)
    ok = True
    # O controle vem primeiro: se o comportamento VELHO nao passar nesta
    # prova, ela nao esta provando nada sobre o novo.
    ok &= rodada(False, "controle: o comportamento de sempre")
    ok &= rodada(True, "pedida a janela")
    shutil.rmtree(BASE, ignore_errors=True)
    print("\n%s" % ("PROVADO: nenhuma linha sumiu dos dois lados"
                    if ok else "REPROVADO"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
