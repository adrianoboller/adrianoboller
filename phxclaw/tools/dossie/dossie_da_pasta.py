#!/usr/bin/env python3
"""Acha o dossie do PhxClaw: UM dono so, por varredura, nunca por nome digitado.

O nome muda a cada refacao (era `dossie-phxclaw.html`, virou `dossie-phxclaw-0.70.html`) e
so existe UM por vez -- o anterior sai no mesmo trabalho, para ninguem atualizar o errado.
Por isso nenhum gerador cita o nome: todos perguntam a esta funcao, que varre
`docs/dossie/dossie-phxclaw-*.html`.

Zero arquivos e parada; dois tambem. Nunca um palpite sobre qual atualizar: e a mesma regra
do `phxsql/docs/dossie/dossie_da_pasta.py`, de onde este desenho veio.

Uso: python3 tools/dossie/dossie_da_pasta.py      (imprime o caminho, ou para com o motivo)
"""
from __future__ import annotations

import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
PASTA = RAIZ / "docs/dossie"
PADRAO = "dossie-phxclaw-*.html"


class SemDono(SystemExit):
    """Parada com motivo: zero ou dois dossies na pasta."""


def candidatos(pasta: Path = PASTA) -> list[Path]:
    return sorted(pasta.glob(PADRAO))


def achar(pasta: Path = PASTA) -> Path:
    achados = candidatos(pasta)
    if len(achados) == 1:
        return achados[0]
    if not achados:
        raise SemDono(
            f"PARADA: nenhum {PADRAO} em {pasta}. O dossie nasce com "
            "`python3 tools/dossie/gerar_dossie.py --novo`, que o nomeia pela versao do Cargo.toml.")
    nomes = ", ".join(p.name for p in achados)
    raise SemDono(
        f"PARADA: {len(achados)} dossies em {pasta} ({nomes}). So existe um por vez: apague o "
        "velho no mesmo trabalho que cria o novo. Nao escolho qual atualizar.")


if __name__ == "__main__":
    print(achar())
    sys.exit(0)
