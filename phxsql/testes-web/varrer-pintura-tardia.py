#!/usr/bin/env python3
"""Varredura da pintura tardia (pedido 636): quem pinta DEPOIS de um `await`
sem conferir a posse do painel.

    python3 phxsql/testes-web/varrer-pintura-tardia.py

Lista as funcoes de `ui/index.html` que tem uma chamada a `folha(` depois de um
`await` e NAO mencionam `aindaNoPainel(`/`tomarPainel(`. Sai com 1 se achar
alguma fora de FALSOS. E uma varredura de TEXTO -- acha a forma esquecida, nao
prova a tela (quem prova e o caso `38-pintura-tardia`). Nao esta ligada a
`bancada/catracas/todas.py`: ligar e decisao do papel G.
"""
import re
import sys
from pathlib import Path

UI = Path(__file__).resolve().parent.parent / "crates/phxsql-server/ui/index.html"

# Falsos positivos medidos em 02/10/2026: duas citacoes em comentario e um
# `onclick` que pinta de proposito uma tela nova.
FALSOS = {"abrirApp", "vPainel", "pivotPasso3"}


def main() -> int:
    linhas = UI.read_text(encoding="utf8").split("\n")
    funcs = [(m.group(2), i) for i, l in enumerate(linhas)
             if (m := re.match(r"^(async )?function (\w+)", l))]
    funcs.append(("", len(linhas)))
    achadas = []
    for (nome, ini), (_, fim) in zip(funcs, funcs[1:]):
        corpo = [(i, linhas[i]) for i in range(ini, fim)
                 if not linhas[i].strip().startswith(("//", "*", "/*"))]
        aw = [i for i, l in corpo if re.search(r"\bawait\b", l)]
        fl = [i for i, l in corpo if re.search(r"\bfolha\(", l)]
        if not aw or not any(f > aw[0] for f in fl):
            continue
        if any(re.search(r"aindaNoPainel\(|tomarPainel\(", l) for _, l in corpo):
            continue
        if nome not in FALSOS:
            achadas.append((nome, ini + 1))
    for nome, n in achadas:
        print(f"pinta depois de um await sem conferir a posse: {nome} (index.html:{n})")
    print(f"{len(achadas)} funcao(oes) fora dos {len(FALSOS)} falsos positivos conhecidos")
    return 1 if achadas else 0


if __name__ == "__main__":
    sys.exit(main())
