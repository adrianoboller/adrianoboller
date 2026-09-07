#!/usr/bin/env python3
"""Prova real dos portoes do medidor comparativo, nos DOIS sentidos.

    python3 bancada/comparativo/prova-dos-portoes.py

Cada portao deste medidor nasceu de um defeito que passou. Portao entregue sem
prova e portao que ninguem sabe se pega -- e nesta casa ja houve teste que
passava por engano, que e pior que teste que falta.

Entao aqui o defeito VOLTA, um por vez, e o medidor tem de PARAR com a
mensagem certa. E o sentido contrario tambem se prova: sem defeito nenhum, o
medidor vai ate o fim e grava o `resultados.json`.
"""

import os
import pathlib
import subprocess
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
MEDIDOR = AQUI / "medir.py"

# defeito -> pedaco que a mensagem de parada TEM de trazer.
ESPERADO = {
    "indice-velho": "MESA NAO POSTA",
    "envelope": "LEITOR QUEBRADO",
    "catalogo-vazio": "SONDA QUEBRADA",
}


def roda(defeito):
    amb = dict(os.environ)
    if defeito:
        amb["PHX_CMP_DEFEITO"] = defeito
    else:
        amb.pop("PHX_CMP_DEFEITO", None)
    r = subprocess.run([sys.executable, str(MEDIDOR)], cwd=RAIZ, env=amb,
                       capture_output=True, text=True)
    return r.returncode, (r.stdout + r.stderr)


def main():
    falhas = []
    for defeito, pedaco in ESPERADO.items():
        codigo, saida = roda(defeito)
        pegou = codigo != 0 and pedaco in saida
        print(f"  {defeito:20} {'PEGOU' if pegou else 'NAO PEGOU'}  "
              f"(codigo {codigo})")
        if not pegou:
            falhas.append(f"{defeito}: esperava parar com {pedaco!r}, "
                          f"saiu {codigo} -- {saida[-300:]}")

    # O outro sentido. Sem ele a prova nao vale: um medidor que para SEMPRE
    # passaria nos tres de cima.
    codigo, saida = roda("")
    limpo = codigo == 0 and "gravado:" in saida
    print(f"  {'sem defeito':20} {'PASSOU' if limpo else 'NAO PASSOU'}  "
          f"(codigo {codigo})")
    if not limpo:
        falhas.append(f"sem defeito: o medidor devia ir ate o fim -- "
                      f"{saida[-300:]}")

    print()
    if falhas:
        for f in falhas:
            print("FALHA:", f)
        sys.exit(1)
    print(f"{len(ESPERADO)} portoes provados nos dois sentidos")


if __name__ == "__main__":
    main()
