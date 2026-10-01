#!/usr/bin/env python3
"""Resolve o conflito de um CATALOGO de entradas com `id` num merge em andamento.

Generalizado do `mesclar_catalogo.py` que o integrador do PhxSql usou para o
catalogo de guardas.

USO (na raiz do repositorio, com o merge parado no conflito)
    mesclar_catalogo.py BRANCH-DA-FRENTE CAMINHO/DO/catalogo.py
    mesclar_catalogo.py BRANCH CAMINHO --bloco REGEX    # outro formato de entrada

A REGRA: parte do HEAD (o lado integrado); traz da frente as entradas NOVAS e
as que SO a frente mudou desde a base comum. Entrada mudada pelos DOIS lados,
de jeitos diferentes, nao se escolhe calado: vai para a mesa (sai com codigo 2,
nomeando o `id`). Por que: o merge textual escolhe um lado inteiro, e foi assim
que um documento perdeu uma secao inteira -- o merge ficou com o lado de quem
nao a tinha.

O FORMATO padrao de entrada e o de uma lista Python de dicionarios com 4
espacos de recuo:

        {
            "id": "nome-da-entrada",
            ...
        },

Para outro formato, passe --bloco com uma regex que case o COMECO da entrada
e capture o id no grupo 1; a entrada termina na proxima ocorrencia de --fim
(padrao "\\n    },\\n"). As novas entram antes do ultimo `]` do arquivo.

Depois: confira o arquivo (o projeto deve ter uma regua de forma do catalogo)
e `git add` pelo caminho.
"""
import argparse
import re
import subprocess
import sys

BLOCO = r'\n    \{\n        (?:#[^\n]*\n        )*"id": "([^"]+)"'
FIM = '\n    },\n'


def git(*args):
    return subprocess.run(("git",) + args, capture_output=True, text=True, check=True).stdout


def blocos(texto, bloco, fim):
    d = {}
    for m in re.finditer(bloco, texto):
        a = m.start() + 1
        b = texto.index(fim, a) + len(fim)
        d[m.group(1)] = texto[a:b]
    return d


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ramo")
    ap.add_argument("caminho", help="caminho do catalogo relativo a raiz do repositorio")
    ap.add_argument("--bloco", default=BLOCO)
    ap.add_argument("--fim", default=FIM)
    a = ap.parse_args()

    base = git("merge-base", "HEAD", a.ramo).strip()
    tb, tt, to = (git("show", "%s:%s" % (r, a.caminho)) for r in (base, a.ramo, "HEAD"))
    b, t, o = (blocos(x, a.bloco, a.fim) for x in (tb, tt, to))

    novos = [k for k in t if k not in b and k not in o]
    mudados_frente = [k for k in t if k in b and t[k] != b[k]]
    mudados_head = {k for k in o if k in b and o[k] != b[k]}
    # Apagada no HEAD e mudada na frente tambem e conflito: o original de onde
    # este saiu dava KeyError nesse caso.
    conflito = [k for k in mudados_frente
                if k not in o or (k in mudados_head and o[k] != t[k])]
    so_frente = [k for k in mudados_frente if k in o and k not in mudados_head]

    for k in so_frente:
        to = to.replace(o[k], t[k])
    k = to.rstrip().rindex("]")
    to = to[:k] + "".join(t[x] for x in novos) + to[k:]
    with open(a.caminho, "w", encoding="utf-8") as fh:
        fh.write(to)

    print("novos:", novos)
    print("mudados so pela frente:", so_frente)
    print("CONFLITO (os dois lados mudaram -- para a mesa):", conflito)
    sys.exit(2 if conflito else 0)


if __name__ == "__main__":
    main()
