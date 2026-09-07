#!/usr/bin/env python3
"""Escreve no dossie a secao das 26 perguntas do dono, com a resposta curta de
cada uma -- lida de `docs/pdf/respostas/*.md`, o MESMO material do PDF.

    python3 docs/dossie/perguntas-no-dossie.py

Item Z do pedido de 07/09/2026: *«atualizacao completa do dossie ... colocando
essas 26 perguntas com as suas devidas respostas»*. A secao mora entre as
marcas `perguntas:inicio`/`perguntas:fim` e NAO se edita: quem quiser mudar
uma palavra mexe na resposta em `docs/pdf/respostas/`, e o PDF e o dossie
mudam juntos -- e a lei da casa contra a segunda copia que diverge.

O que sai daqui e a RESPOSTA CURTA de cada item (o bloco entre `## Resposta
curta` e o `##` seguinte), mais a lista do que falta. O exemplo exercitado
inteiro fica no PDF, que e onde cabe; o dossie aponta para ele.

Acha o dossie pelo `dossie_da_pasta` (um dono so, nunca um nome digitado), e
reaproveita o conversor de Markdown do `docs/pdf/gerar.py` -- um conversor,
nao dois, pelo mesmo motivo de uma resposta e nao duas.
"""
import importlib.util
import pathlib
import re
import subprocess
import sys
import time

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
RESPOSTAS = RAIZ / "docs" / "pdf" / "respostas"

sys.path.insert(0, str(AQUI))
from dossie_da_pasta import achar_o_dossie  # noqa: E402

ABRE = "<!-- perguntas:inicio (gerado por docs/dossie/perguntas-no-dossie.py) -->"
FECHA = "<!-- perguntas:fim -->"


def conversor():
    spec = importlib.util.spec_from_file_location("gerar_pdf", RAIZ / "docs" / "pdf" / "gerar.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def resposta_curta(md: str) -> str:
    """O bloco entre `## Resposta curta` e o proximo `##`, em Markdown."""
    m = re.search(r"^## Resposta curta\s*\n(.*?)(?=^## |\Z)", md, re.S | re.M)
    return m.group(1).strip() if m else ""


def main() -> int:
    dossie = achar_o_dossie()
    txt = dossie.read_text(encoding="utf-8")
    i, j = txt.find(ABRE), txt.find(FECHA)
    if i < 0 or j < 0:
        raise SystemExit(f"{dossie.name} nao tem as marcas perguntas:inicio/fim -- a secao precisa existir uma vez, com as marcas, antes de este gerador escrever nela")
    g = conversor()
    arquivos = sorted(RESPOSTAS.glob("*.md"), key=g.ordem)
    letras = [chr(c) for c in range(ord("A"), ord("Z") + 1)]
    presentes = {p.stem for p in arquivos}
    faltam = [L for L in letras if L not in presentes]
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=RAIZ, capture_output=True, text=True).stdout.strip()
    agora = time.strftime("%d/%m/%Y %H:%M UTC", time.gmtime())

    blocos = []
    for p in arquivos:
        md = p.read_text(encoding="utf-8")
        titulo = next((l[2:].strip() for l in md.splitlines() if l.startswith("# ")), p.stem)
        _, corpo = g.md_para_html(resposta_curta(md))
        blocos.append(f'<details class="pergunta"><summary>{g.inline(titulo)}</summary>{corpo}'
                      f'<p class="leve">Exemplo exercitado, saída real e como se refaz: <code>docs/pdf/respostas/{p.name}</code>, no PDF.</p></details>')
    aviso = ""
    if faltam:
        aviso = (f'<p><strong>Ainda sem resposta nesta geração:</strong> {", ".join(faltam)} — '
                 "item que falta aparece como faltando, em vez de sumir da lista.</p>")
    bloco = (f'\n  <p class="leve">{len(arquivos)} respostas, lidas de <code>docs/pdf/respostas/</code> em {agora} '
             f'(commit <code>{commit}</code>). O PDF completo — comando, saída real e data de cada corrida — é '
             f'<code>docs/pdf/phxsql-26-perguntas.pdf</code>, gerado por <code>docs/pdf/gerar.py</code>.</p>\n  {aviso}\n  '
             + "\n  ".join(blocos) + "\n")
    novo = txt[:i] + ABRE + bloco + FECHA + txt[j + len(FECHA):]
    dossie.write_text(novo, encoding="utf-8")
    print(f"{dossie.name}: secao das perguntas regravada -- {len(arquivos)} respostas"
          + (f", faltam {len(faltam)}: {' '.join(faltam)}" if faltam else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
