#!/usr/bin/env python3
"""Quanto custa, por evento, levar a posicao do bidirecional ao disco DEPOIS
do dado (pedido 535) -- medido, e nao suposto.

    cargo build --release
    python3 bancada/replicacao/custo-da-posicao.py [--phxsqld BIN] [--n N] [--voltas V]

O cenario e o do alcance inteiro, que e onde o conserto mexe: `beta` recebe N
linhas com `alfa` DESLIGADO; `alfa` sobe puxando de `beta`, e o relogio conta
da subida ate as N linhas aparecerem em `alfa`. Um alcance so, varios lotes.

O que o conserto muda nesse caminho, e por isso e o que se mede:

    antes  -- um `write` sem `fsync` da posicao a CADA lote, e o `fsync` da
              tabela no fim;
    depois -- o `fsync` da tabela no fim e UMA troca duravel da posicao
              (`fsync` do temporario, `rename`, `fsync` da pasta).

Para comparar os dois, rode duas vezes com binarios diferentes (`--phxsqld`).
O tempo inclui subir o processo e o primeiro intervalo do laco -- os dois
lados pagam o mesmo --, e por isso a conta util e a DIFERENCA das medianas,
com a faixa min-max ao lado (o pedido 155: sem faixa nao ha vencedor).

So derruba processos que ele mesmo criou (os do `modos.py`, que guarda os
Popen). Portas 5336-5337. A ultima linha e `RESULTADO <json>`.
"""
import argparse
import json
import os
import statistics
import sys
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import modos  # noqa: E402  -- os ajudantes de soquete e de processo, um motor so

PORTA_ALFA, PORTA_BETA = 5336, 5337
# Teto de voltas de espera: nenhum laco desta bancada roda sem fim.
ESPERA_S = 120


def config_alfa(h, com_origem):
    # O papel `multi` exige origem: o primeiro arranque, so para criar a
    # tabela, sobe sem replicacao nenhuma.
    if not com_origem:
        return modos.config_base(PORTA_ALFA, h)
    return {**modos.config_base(PORTA_ALFA, h), "replicacao": {
        "papel": "multi", "id_servidor": "alfa", "imagem_da_linha": True,
        "origens": [modos.origem_para(PORTA_BETA, "beta", h)]}}


def uma_volta(base, h, n):
    # A tabela de `alfa` nasce ANTES, com ele sem origem: senao a primeira
    # rodada acharia a tabela faltando e o relogio pagaria um intervalo
    # inteiro do laco, que nao tem nada a ver com a posicao.
    modos.subir(base, "p-alfa", config_alfa(h, False))
    modos.criar_tabela(modos.liga(PORTA_ALFA), "clientes")
    modos.derrubar("p-alfa")
    # `beta` tambem precisa de origem (o `multi` exige): aponta para `alfa`,
    # que esta desligado enquanto as linhas entram -- e o par de sempre.
    modos.subir(base, "p-beta", {**modos.config_base(PORTA_BETA, h), "replicacao": {
        "papel": "multi", "id_servidor": "beta", "imagem_da_linha": True,
        "origens": [modos.origem_para(PORTA_ALFA, "alfa", h)]}})
    b = modos.liga(PORTA_BETA)
    modos.criar_tabela(b, "clientes")
    for i in range(1, n + 1):
        modos.inserir(b, "clientes", i, f"linha {i}")
    t0 = time.perf_counter()
    # `subir` apaga a `base` e a posicao: aqui so a posicao pode sair, entao
    # o segundo arranque de `alfa` reusa o diretorio por outro rotulo.
    d = os.path.join(base, "p-alfa")
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(config_alfa(h, True), f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = modos.subprocess.Popen([modos.PHXSQLD], cwd=d, stdout=log,
                               stderr=modos.subprocess.STDOUT,
                               stdin=modos.subprocess.DEVNULL)
    modos.PROCESSOS.append((p, "p-alfa"))
    a = modos.liga(PORTA_ALFA)
    dt = modos.esperar(lambda: len(modos.linhas_por_id(a, "clientes") or {}) >= n,
                       ESPERA_S, 0.05)
    if dt is not None:
        dt = time.perf_counter() - t0
    modos.derrubar("p-alfa", "p-beta")
    return dt


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--phxsqld", default=modos.PHXSQLD)
    ap.add_argument("--n", type=int, default=5000)
    ap.add_argument("--voltas", type=int, default=5)
    ap.add_argument("--rotulo", default="")
    args = ap.parse_args()
    modos.PHXSQLD = args.phxsqld
    h = modos.hash_da_senha(modos.SENHA)
    tempos = []
    with tempfile.TemporaryDirectory(prefix="custo-da-posicao-") as base:
        try:
            for v in range(args.voltas):
                dt = uma_volta(os.path.join(base, f"v{v}"), h, args.n)
                print(f"   volta {v + 1}: {'NAO ALCANCOU' if dt is None else f'{dt:.3f} s'}")
                if dt is not None:
                    tempos.append(dt)
        finally:
            modos.derrubar()
    if not tempos:
        print("RESULTADO " + json.dumps({"ok": False}))
        return 1
    med = statistics.median(tempos)
    r = {
        "ok": True,
        "rotulo": args.rotulo,
        "binario": args.phxsqld,
        "n": args.n,
        "voltas": len(tempos),
        "mediana_s": round(med, 4),
        "min_s": round(min(tempos), 4),
        "max_s": round(max(tempos), 4),
        "us_por_evento_mediana": round(med / args.n * 1e6, 2),
    }
    print(f"   mediana {med:.3f} s (min {min(tempos):.3f}, max {max(tempos):.3f}) "
          f"= {r['us_por_evento_mediana']} us/evento, n={args.n}")
    print("RESULTADO " + json.dumps(r))
    return 0


if __name__ == "__main__":
    sys.exit(main())
