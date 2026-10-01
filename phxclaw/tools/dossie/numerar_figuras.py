#!/usr/bin/env python3
"""Numera TODAS as figuras do dossie na ordem do documento, e as referencias a elas.

Licao do PhxSql (`numerar-figuras.py`): enquanto figura so entrava no fim, o numero digitado
batia por sorte; duas no meio envelheceram dezesseis legendas de uma vez. Legenda errada nao
quebra nada -- a pagina abre, o desenho aparece --, e por isso ninguem confere.

Contrato com quem escreve o HTML:
  - figura:     <figure id="fig-NOME" ...> ... <b class="fig-n">Figura ?.</b> ... </figure>
  - referencia: <a class="ref-fig" href="#fig-NOME">Figura ?</a>
O numero dentro das duas e reescrito aqui; quem escreve pode deixar qualquer coisa.

PARA (sai != 0) quando: figura sem legenda numeravel, legenda numeravel fora de figura,
id repetido, ou referencia a figura que nao existe. Referencia morta e a «chave morta» da
fabrica de idiomas: quem le acha que ha uma figura que nao ha.

Idempotente: rodar duas vezes nao muda nenhum byte.

Uso: python3 tools/dossie/numerar_figuras.py [ARQ.html]   (sem argumento: o dossie da pasta)
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

FIGURA = re.compile(r'<figure\b[^>]*\bid="(fig-[a-z0-9-]+)"[^>]*>(.*?)</figure>', re.S)
LEGENDA = re.compile(r'<b class="fig-n">Figura [^<]*</b>')
REF = re.compile(r'<a class="ref-fig" href="#(fig-[a-z0-9-]+)">Figura [^<]*</a>')


class FiguraErrada(SystemExit):
    pass


def numerar(html: str) -> tuple[str, dict[str, int]]:
    numeros: dict[str, int] = {}

    def troca(m: re.Match) -> str:
        fid, corpo = m.group(1), m.group(2)
        if fid in numeros:
            raise FiguraErrada(f"PARADA: id de figura repetido: {fid}")
        if len(LEGENDA.findall(corpo)) != 1:
            raise FiguraErrada(f"PARADA: a figura {fid} precisa de UMA legenda <b class=\"fig-n\">")
        numeros[fid] = len(numeros) + 1
        corpo = LEGENDA.sub(f'<b class="fig-n">Figura {numeros[fid]}.</b>', corpo)
        return m.group(0)[: m.start(2) - m.start(0)] + corpo + "</figure>"

    saida = FIGURA.sub(troca, html)
    soltas = len(LEGENDA.findall(saida)) - len(numeros)
    if soltas:
        raise FiguraErrada(f"PARADA: {soltas} legenda(s) numeravel(is) fora de <figure id=\"fig-...\">")

    def ref(m: re.Match) -> str:
        fid = m.group(1)
        if fid not in numeros:
            raise FiguraErrada(f"PARADA: referencia a figura inexistente: #{fid}")
        return f'<a class="ref-fig" href="#{fid}">Figura {numeros[fid]}</a>'

    return REF.sub(ref, saida), numeros


def main() -> int:
    if len(sys.argv) > 1:
        alvo = Path(sys.argv[1])
    else:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from dossie_da_pasta import achar
        alvo = achar()
    antes = alvo.read_text(encoding="utf-8")
    depois, numeros = numerar(antes)
    if depois != antes:
        alvo.write_text(depois, encoding="utf-8")
    print(f"{alvo.name}: {len(numeros)} figuras numeradas"
          + ("" if depois != antes else " (nenhuma mudou)"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
