#!/usr/bin/env python3
"""Gera o PDF das 26 perguntas a partir de `docs/pdf/respostas/*.md`.

    python3 docs/pdf/gerar.py            # escreve o .html e imprime o .pdf
    python3 docs/pdf/gerar.py --so-html  # so o .html (para olhar no navegador)

So biblioteca padrao + o Chromium que o Playwright ja instalou em
/opt/pw-browsers (`--headless --print-to-pdf`). Nenhuma dependencia nova: e a
lei da casa, e e o que faz isto rodar em qualquer sessao.

O que este gerador NAO faz, de proposito: nao digita numero nenhum. A data, o
commit, a contagem de respostas e a lista de perguntas saem dos arquivos e do
`git`. Quem quiser mudar uma palavra mexe na resposta; quem quiser mudar a
aparencia mexe no CSS daqui. O `.html` e o `.pdf` nao se editam.

O conversor de Markdown e um subconjunto -- o que o contrato do
`docs/pdf/LEIA-ME.md` usa: titulos, paragrafos, cerca de codigo, codigo em
linha, negrito, italico, tabela, lista, imagem, link, citacao. Markdown fora
disso sai como paragrafo cru, que e visivel -- melhor que um conversor generico
que esconde o que nao entende.
"""
from __future__ import annotations

import html
import re
import subprocess
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
RESPOSTAS = AQUI / "respostas"
SAIDA_HTML = AQUI / "phxsql-26-perguntas.html"
SAIDA_PDF = AQUI / "phxsql-26-perguntas.pdf"
CHROME = Path("/opt/pw-browsers/chromium-1194/chrome-linux/chrome")


# ------------------------------------------------------------ markdown

def inline(s: str) -> str:
    """Negrito, italico, codigo, link e imagem dentro de uma linha."""
    s = html.escape(s, quote=False)
    # O codigo em linha sai de cena antes das outras marcas e volta depois:
    # cortar a linha nos `codigos` e formatar cada pedaco separava os dois
    # asteriscos de um **negrito com `codigo` dentro**, e ele saia cru.
    codigos: list[str] = []
    def guarda(m):
        codigos.append(f"<code>{m.group(1)}</code>")
        return f"\x00{len(codigos) - 1}\x00"
    s = re.sub(r"`([^`]+)`", guarda, s)
    s = re.sub(r"!\[([^\]]*)\]\(([^)]+)\)", r'<img src="\2" alt="\1">', s)
    s = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', s)
    s = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", s)
    s = re.sub(r"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])", r"<i>\1</i>", s)
    return re.sub(r"\x00(\d+)\x00", lambda m: codigos[int(m.group(1))], s)


def tabela(linhas: list[str]) -> str:
    cel = lambda l: [c.strip() for c in l.strip().strip("|").split("|")]
    cab = cel(linhas[0])
    corpo = [cel(l) for l in linhas[2:]]
    out = ["<table><thead><tr>"] + [f"<th>{inline(c)}</th>" for c in cab] + ["</tr></thead><tbody>"]
    for r in corpo:
        out.append("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>")
    out.append("</tbody></table>")
    return "".join(out)


def md_para_html(texto: str) -> tuple[str, str]:
    """Devolve (titulo_h1, html do corpo)."""
    linhas = texto.splitlines()
    out: list[str] = []
    titulo = ""
    i = 0
    par: list[str] = []

    def fecha_par():
        nonlocal par
        if par:
            txt = ' '.join(par)
            # A linha de carimbo do contrato ("*Medido em ..., commit `x`.*"): o
            # italico por regex nao a alcanca porque o `codigo` no meio parte a
            # linha em pedacos. Ela ganha classe propria, que e o que se queria.
            if txt.startswith("*") and txt.endswith("*") and txt.count("*") == 2:
                out.append(f'<p class="medido">{inline(txt[1:-1])}</p>')
            else:
                out.append(f"<p>{inline(txt)}</p>")
            par = []

    while i < len(linhas):
        l = linhas[i]
        if l.startswith("```"):
            fecha_par()
            lang = l[3:].strip()
            j = i + 1
            bloco = []
            while j < len(linhas) and not linhas[j].startswith("```"):
                bloco.append(linhas[j]); j += 1
            cls = f' class="lang-{html.escape(lang)}"' if lang else ""
            out.append(f"<pre{cls}><code>{html.escape(chr(10).join(bloco), quote=False)}</code></pre>")
            i = j + 1
            continue
        if l.startswith("#"):
            fecha_par()
            n = len(l) - len(l.lstrip("#"))
            t = l[n:].strip()
            if n == 1 and not titulo:
                titulo = t
            else:
                out.append(f"<h{min(n,4)}>{inline(t)}</h{min(n,4)}>")
            i += 1
            continue
        if l.strip().startswith("|") and i + 1 < len(linhas) and re.match(r"^\s*\|?\s*:?-{2,}", linhas[i + 1]):
            fecha_par()
            j = i
            bloco = []
            while j < len(linhas) and linhas[j].strip().startswith("|"):
                bloco.append(linhas[j]); j += 1
            out.append(tabela(bloco))
            i = j
            continue
        if re.match(r"^\s*[-*] ", l) or re.match(r"^\s*\d+[.)] ", l):
            fecha_par()
            ordenada = bool(re.match(r"^\s*\d+[.)] ", l))
            tag = "ol" if ordenada else "ul"
            itens = []
            j = i
            while j < len(linhas) and (re.match(r"^\s*[-*] ", linhas[j]) or re.match(r"^\s*\d+[.)] ", linhas[j]) or (linhas[j].startswith("  ") and linhas[j].strip())):
                if re.match(r"^\s*([-*]|\d+[.)]) ", linhas[j]):
                    itens.append(re.sub(r"^\s*([-*]|\d+[.)]) ", "", linhas[j]))
                else:
                    itens[-1] += " " + linhas[j].strip()
                j += 1
            out.append(f"<{tag}>" + "".join(f"<li>{inline(x)}</li>" for x in itens) + f"</{tag}>")
            i = j
            continue
        if l.startswith(">"):
            fecha_par()
            j = i; bloco = []
            while j < len(linhas) and linhas[j].startswith(">"):
                bloco.append(linhas[j][1:].strip()); j += 1
            out.append(f"<blockquote>{inline(' '.join(bloco))}</blockquote>")
            i = j
            continue
        if not l.strip():
            fecha_par(); i += 1; continue
        par.append(l.strip()); i += 1
    fecha_par()
    return titulo, "\n".join(out)


# ------------------------------------------------------------ montagem

CSS = """
@page { size: A4; margin: 18mm 16mm 20mm 16mm; }
:root{ --papel:#fbf9f7; --tinta:#1b1a17; --acento:#c63c0a; --azul:#1f5c93; --linha:#d8d3cc; --cinza:#6b675f; --fundo-cod:#f1eee8; }
html,body{ background:#fff; color:var(--tinta); }
body{ font-family:"Source Serif 4", Georgia, "Times New Roman", serif; font-size:10.5pt; line-height:1.45; margin:0; }
h1,h2,h3,h4{ font-family:"Exo 2","Helvetica Neue",Arial,sans-serif; color:var(--tinta); line-height:1.2; text-wrap:balance; }
h1{ font-size:19pt; margin:0 0 6pt; border-bottom:2px solid var(--acento); padding-bottom:6pt; }
h2{ font-size:13pt; margin:16pt 0 4pt; color:var(--acento); }
h3{ font-size:11.5pt; margin:12pt 0 3pt; }
p{ margin:0 0 7pt; }
code{ font-family:"IBM Plex Mono", Menlo, Consolas, monospace; font-size:9pt; background:var(--fundo-cod); padding:0 3px; border-radius:2px; }
pre{ background:var(--fundo-cod); border:1px solid var(--linha); border-left:3px solid var(--azul); padding:7pt 9pt; margin:6pt 0 9pt; white-space:pre-wrap; word-break:break-word; font-size:8.6pt; line-height:1.35; page-break-inside:avoid; }
pre code{ background:none; padding:0; font-size:inherit; }
table{ border-collapse:collapse; width:100%; margin:6pt 0 9pt; font-size:9.5pt; page-break-inside:avoid; }
th,td{ border:1px solid var(--linha); padding:3pt 6pt; text-align:left; vertical-align:top; }
th{ font-family:"Exo 2",Arial,sans-serif; font-weight:600; background:var(--fundo-cod); }
td{ font-variant-numeric:tabular-nums; }
ul,ol{ margin:0 0 8pt 18pt; padding:0; } li{ margin:0 0 3pt; }
blockquote{ margin:6pt 0 9pt; padding:4pt 10pt; border-left:3px solid var(--acento); color:var(--cinza); }
img{ max-width:100%; height:auto; border:1px solid var(--linha); display:block; margin:6pt 0 9pt; page-break-inside:avoid; }
a{ color:var(--azul); text-decoration:none; }
.capa{ page-break-after:always; padding-top:40mm; }
.capa .marca{ font-family:"Exo 2",Arial,sans-serif; font-weight:700; font-size:34pt; letter-spacing:-.01em; color:#010418; margin:0; }
.capa .assina{ font-family:"Exo 2",Arial,sans-serif; color:var(--acento); font-size:12pt; margin:2pt 0 28pt; }
.capa h1{ border:0; font-size:22pt; }
.capa .meta{ color:var(--cinza); font-size:9.5pt; margin-top:24pt; }
.indice{ page-break-after:always; }
.indice ol{ columns:2; column-gap:24pt; font-size:9.5pt; margin-left:14pt; }
.indice li{ break-inside:avoid; margin-bottom:4pt; }
.item{ page-break-before:always; }
.item .medido{ color:var(--cinza); font-size:9pt; margin:-2pt 0 10pt; }
.rodape{ font-family:"Exo 2",Arial,sans-serif; font-size:8pt; color:var(--cinza); border-top:1px solid var(--linha); margin-top:14pt; padding-top:4pt; }
"""


def ordem(p: Path) -> tuple[int, str]:
    n = p.stem
    return (0 if n.startswith("00-") else 1, n)


def main() -> int:
    so_html = "--so-html" in sys.argv
    arquivos = sorted(RESPOSTAS.glob("*.md"), key=ordem)
    if not arquivos:
        print("nenhuma resposta em docs/pdf/respostas/ -- nada a gerar", file=sys.stderr)
        return 2
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=RAIZ, capture_output=True, text=True).stdout.strip()
    agora = time.strftime("%d/%m/%Y %H:%M UTC", time.gmtime())

    secoes = []
    indice = []
    faltam = []
    letras_esperadas = [chr(c) for c in range(ord("A"), ord("Z") + 1)]
    presentes = {p.stem for p in arquivos}
    for L in letras_esperadas:
        if L not in presentes:
            faltam.append(L)
    for p in arquivos:
        titulo, corpo = md_para_html(p.read_text(encoding="utf-8"))
        anc = "r-" + re.sub(r"[^a-z0-9]+", "-", p.stem.lower())
        indice.append(f'<li><a href="#{anc}">{inline(titulo)}</a></li>')
        secoes.append(f'<section class="item" id="{anc}"><h1>{inline(titulo)}</h1>{corpo}'
                      f'<div class="rodape">PhxSql — {inline(titulo)[:80]} · commit {commit} · gerado por docs/pdf/gerar.py em {agora}</div></section>')

    aviso_faltam = ""
    if faltam:
        aviso_faltam = (f'<p class="meta"><b>Respostas ainda não escritas nesta geração:</b> {", ".join(faltam)}. '
                        "Item que falta aparece como faltando, em vez de sumir do índice — papel que não está cumprindo aparece como não cumprindo.</p>")

    pagina = f"""<!doctype html><html lang="pt-BR"><head><meta charset="utf-8">
<title>PhxSql — as 26 perguntas</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>{CSS}</style></head><body>
<section class="capa">
  <p class="marca">PhxSql</p>
  <p class="assina">Built to store. Engineered to scale.</p>
  <h1>As 26 perguntas — com o exemplo exercitado de cada uma</h1>
  <p>Pedido do dono em 07/09/2026: três perguntas abertas e vinte e seis itens, de A a Z, cada um respondido
  contra o motor vivo — comando, saída real, data da corrida e o que <b>não</b> existe, dito com todas as letras.</p>
  <p class="meta">{len(arquivos)} respostas · commit <code>{commit}</code> · gerado em {agora} por <code>docs/pdf/gerar.py</code>.
  Nenhum número deste documento foi digitado: cada um saiu de uma corrida datada, e o comando que a refaz está no fim de cada resposta.</p>
  {aviso_faltam}
</section>
<section class="indice"><h1>Índice</h1><ol>{"".join(indice)}</ol></section>
{"".join(secoes)}
</body></html>"""
    SAIDA_HTML.write_text(pagina, encoding="utf-8")
    print(f"html: {SAIDA_HTML.relative_to(RAIZ)} ({SAIDA_HTML.stat().st_size:,} bytes, {len(arquivos)} respostas"
          + (f", faltam {len(faltam)}: {' '.join(faltam)}" if faltam else "") + ")")
    if so_html:
        return 0
    if not CHROME.exists():
        print(f"chromium nao esta em {CHROME}; o .html ficou pronto e o .pdf nao foi gerado", file=sys.stderr)
        return 3
    r = subprocess.run([str(CHROME), "--headless=new", "--no-sandbox", "--disable-gpu",
                        "--no-pdf-header-footer", f"--print-to-pdf={SAIDA_PDF}",
                        "--virtual-time-budget=8000", SAIDA_HTML.as_uri()],
                       capture_output=True, text=True, timeout=180)
    if not SAIDA_PDF.exists():
        print("o chromium nao escreveu o pdf:\n" + r.stderr[-800:], file=sys.stderr)
        return 4
    dados = SAIDA_PDF.read_bytes()
    paginas = len(re.findall(rb"/Type\s*/Page[^s]", dados))
    print(f"pdf:  {SAIDA_PDF.relative_to(RAIZ)} ({len(dados):,} bytes, {paginas} paginas contadas pela marca /Type /Page)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
