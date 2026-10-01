#!/usr/bin/env python3
"""Pedido 605, medicao M2: o «ok» de um TERCEIRO sai antes ou depois do
`fsync` da pasta de quem criou a tabela?

    python3 bancada/durabilidade/terceiro-605.py --binario target/debug/phxsqld \
        --rotulo depois

# O que se mede, e por que sem queda nenhuma

A cria a tabela `nova`; B, outra conexao, insere nela assim que o `.reg`
aparece no disco. O `strace` ANEXADO ao servidor ATRASA so o `fsync` da pasta
do database (`-P <pasta>` com `-e inject=fsync:delay_enter=...`), e com isso a
janela entre «a tabela ja e visivel» e «a entrada dela esta no disco» passa de
microssegundos para o atraso pedido. O veredito e uma comparacao de RELOGIO:

* VERMELHO -- o «ok» de B chegou ANTES do fim desse `fsync`: B ouviu que a
  linha dele esta no disco enquanto a entrada da tabela onde ela mora ainda
  nao estava (ext4 sem diario: a queda leva as duas -- `docs/propostas/
  605-medicao-segura-01-10-2026.md` §2);
* VERDE -- o «ok» de B so chegou depois.

Nada aqui derruba sistema de arquivos, monta coisa alguma ou mexe com ioctl:
e um `ptrace` em cima de um processo nosso, e o atraso e um `nanosleep` antes
da chamada. O que esta medicao NAO prova e perda em disco -- prova a ORDEM das
respostas, que e a pergunta do 605 sobre o nosso codigo (M2 do desenho).

# O filtro por caminho, conferido

`-P` restringe o rastreio E a injecao: so a chamada sobre a pasta entra no
atraso. Conferido antes de usar (strace 6.8): com `-P d`, um `fsync` no
arquivo `d/x` levou 0,001 s e o da pasta `d` levou 0,701 s com
`delay_enter=700000`. Sem isso o atraso cairia tambem nos `fsync` do proprio
B, e a medicao nao diria nada.

Mata so o PID que ele mesmo subiu.
"""
import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))
sys.path.insert(0, AQUI)
import comum  # noqa: E402
import prova as dur  # noqa: E402

DB = "t605"
PORTA = 7605

# A linha inteira de um `fsync` sob `-ttt -T -y`, e as duas metades da que o
# `strace` partiu porque outra thread entrou no meio. A chamada atrasada sai
# com `(DELAYED)` entre o resultado e a duracao -- a primeira versao destas
# expressoes nao o previa e nao achava fsync nenhum.
INTEIRA = re.compile(r"^\s*(\d+)\s+([\d.]+)\s+fsync\(\d+<([^>]+)>\)\s*=\s*(-?\d+)(?:\s+\(DELAYED\))?\s+<([\d.]+)>")
ABERTA = re.compile(r"^\s*(\d+)\s+([\d.]+)\s+fsync\(\d+<([^>]+)>\s+<unfinished")
FECHADA = re.compile(r"^\s*(\d+)\s+([\d.]+)\s+<\.\.\.\s+fsync\s+resumed>\)?\s*=\s*(-?\d+)(?:\s+\(DELAYED\))?\s+<([\d.]+)>")


def fsyncs_da_pasta(arquivo):
    """(inicio, fim, caminho) de cada `fsync` do traco. O inicio e o carimbo
    da entrada; o fim e entrada + duracao (`-T`), que inclui o atraso."""
    abertas = {}
    saida = []
    with open(arquivo, errors="replace") as f:
        for linha in f:
            m = INTEIRA.match(linha)
            if m:
                ini = float(m.group(2))
                saida.append((ini, ini + float(m.group(5)), m.group(3)))
                continue
            m = ABERTA.match(linha)
            if m:
                abertas[m.group(1)] = (float(m.group(2)), m.group(3))
                continue
            m = FECHADA.match(linha)
            if m and m.group(1) in abertas:
                ini, caminho = abertas.pop(m.group(1))
                saida.append((ini, float(m.group(2)), caminho))
    return sorted(saida)


def anexar(pid, pasta, atraso_us, arquivo):
    p = subprocess.Popen(
        ["strace", "-f", "-y", "-ttt", "-T", "-P", pasta, "-e", "trace=fsync",
         "-e", "inject=fsync:delay_enter=%d" % atraso_us, "-o", arquivo,
         "-p", str(pid)],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.8)  # o attach assentar em todas as threads existentes
    if p.poll() is not None:
        raise SystemExit("strace morreu ao anexar no pid %d" % pid)
    return p


def soltar(p):
    p.send_signal(signal.SIGINT)
    try:
        p.wait(5)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait(5)
    time.sleep(0.2)


def uma_corrida(binario, atraso_us, base):
    comum.PHXSQLD = binario
    dur.PHXSQLD = binario
    dur.PORTA = PORTA
    dur.BASE = base
    p, _ = dur.subir(dur.config("por_operacao"), limpar=True)
    st = None
    traco = os.path.join(base, "strace-605.txt")
    try:
        c0 = dur.Ligacao(porta=PORTA)
        c0.ok({"op": "criar_database", "database": DB})
        pasta = os.path.join(base, "base", DB)
        if not os.path.isdir(pasta):
            raise SystemExit("a pasta do database nao esta onde se esperava: %s" % pasta)
        ca = dur.Ligacao(porta=PORTA)
        cb = dur.Ligacao(porta=PORTA)
        st = anexar(p.pid, pasta, atraso_us, traco)
        tempos = {}

        def lado_a():
            tempos["a0"] = time.time()
            tempos["ra"] = ca.fala({
                "op": "criar_tabela", "database": DB, "tabela": "nova",
                "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True}],
                "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                             "primario": True}]})
            tempos["a"] = time.time()

        ta = threading.Thread(target=lado_a)
        ta.start()
        reg = os.path.join(pasta, "nova.reg")
        prazo = time.time() + 10
        while not os.path.exists(reg) and time.time() < prazo:
            time.sleep(0.0005)
        tempos["b0"] = time.time()
        rb = cb.fala({"op": "inserir", "database": DB, "tabela": "nova",
                      "linha": {"id": 1}})
        tempos["b"] = time.time()
        ta.join(30)
        time.sleep(0.3)
        soltar(st)
        st = None
        for c in (c0, ca, cb):
            c.fechar()
    finally:
        if st is not None:
            soltar(st)
        dur.derrubar_limpo(p)
    da_pasta = [(i, f) for i, f, c in fsyncs_da_pasta(traco)
                if os.path.realpath(c) == os.path.realpath(pasta) and i >= tempos["a0"]]
    if not da_pasta:
        raise SystemExit("o traco nao tem o fsync da pasta depois do criar: %s" % traco)
    ini, fim = da_pasta[0]
    a0 = tempos["a0"]
    ms = lambda t: round((t - a0) * 1000, 1)  # noqa: E731
    return {
        "ok_a": bool(tempos["ra"].get("ok")),
        "ok_b": bool(rb.get("ok")),
        "erro_b": rb.get("erro"),
        "fsync_da_pasta_inicio_ms": ms(ini),
        "fsync_da_pasta_fim_ms": ms(fim),
        "b_pediu_ms": ms(tempos["b0"]),
        "ok_de_b_ms": ms(tempos["b"]),
        "ok_de_a_ms": ms(tempos["a"]),
        "fsyncs_da_pasta_no_traco": len(da_pasta),
        "veredito": "VERDE" if tempos["b"] >= fim else "VERMELHO",
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--binario", default=os.path.join(RAIZ, "target", "release", "phxsqld"))
    ap.add_argument("--atraso-ms", type=int, default=1500)
    ap.add_argument("--corridas", type=int, default=3)
    ap.add_argument("--rotulo", default="")
    ap.add_argument("--json", default="")
    a = ap.parse_args()
    # FORA da arvore: o servidor grava a chave do desafio na base, e o
    # conferidor de segredos reprova arquivo de chave solto no repositorio.
    base = tempfile.mkdtemp(prefix="phx-terceiro-605-")
    corridas = []
    try:
        for n in range(a.corridas):
            r = uma_corrida(os.path.abspath(a.binario), a.atraso_ms * 1000, base)
            corridas.append(r)
            print("corrida %d: %s  fsync da pasta de A termina em %.1f ms; ok de B em %.1f ms; "
                  "ok de A em %.1f ms (B ok=%s)" % (
                      n + 1, r["veredito"], r["fsync_da_pasta_fim_ms"], r["ok_de_b_ms"],
                      r["ok_de_a_ms"], r["ok_b"]))
    finally:
        shutil.rmtree(base, ignore_errors=True)
    vermelhas = sum(1 for r in corridas if r["veredito"] == "VERMELHO")
    print("%s: %d/%d VERMELHO (ok de B antes do fsync da pasta de A)" % (
        a.rotulo or "corrida", vermelhas, len(corridas)))
    if a.json:
        with open(a.json, "w") as f:
            json.dump({"rotulo": a.rotulo, "atraso_ms": a.atraso_ms,
                       "medido_em": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
                       "corridas": corridas}, f, indent=2)
            f.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
