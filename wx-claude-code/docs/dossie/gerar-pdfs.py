#!/usr/bin/env python3
"""Gera os PDFs de documentacao (manual, fluxograma, workflow, apresentacao) das fontes.

Os quatro PDFs ficaram um dia inteiro atras do MANUAL.md e do fluxo-atual.html
porque nao tinham gerador: eram feitos a mao numa sessao e commitados. E o mesmo
defeito do numero digitado -- envelhece calado --, agora num documento que o
cliente recebe no zip.

Cada PDF so e refeito quando a FONTE dele muda (SHA-256 guardado em pdfs.json):
sem isso, toda rodada do atualizar-paginas trocaria os bytes do PDF e sujaria o
commit. O Markdown vira HTML pelo modulo `markdown` do Python; sem ele, o script
diz INDISPONIVEL e sai com 1 em vez de deixar o PDF velho passar por novo.

Uso: python3 docs/dossie/gerar-pdfs.py [--forcar] [--conferir]
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
DOSSIE = RAIZ / "docs/dossie"
ESTADO = DOSSIE / "pdfs.json"
PLAYWRIGHT = "/opt/node22/lib/node_modules/playwright/index.mjs"

# (fonte, pdf); a fonte .md vira HTML antes de imprimir
ALVOS = [
    (RAIZ / "MANUAL.md", RAIZ / "docs/manual-de-uso.pdf"),
    (DOSSIE / "fluxo-atual.html", DOSSIE / "fluxo-atual.pdf"),
    (RAIZ / "docs/workflow.html", RAIZ / "docs/workflow.pdf"),
    (RAIZ / "docs/apresentacao.html", RAIZ / "docs/apresentacao.pdf"),
]

CSS_MD = """<meta charset="utf-8"><style>
body{font-family:"Source Serif 4",Georgia,serif;font-size:11pt;line-height:1.45;color:#14161F;max-width:none;margin:0}
h1{font-family:"Exo 2","Segoe UI",sans-serif;color:#C63C0A;font-size:22pt;margin:0 0 8pt}
h2{font-family:"Exo 2","Segoe UI",sans-serif;font-size:15pt;margin:18pt 0 6pt;border-bottom:1px solid #D9D6CE;padding-bottom:3pt;page-break-after:avoid}
h3{font-size:12pt;margin:12pt 0 4pt;page-break-after:avoid}
code{font-family:"JetBrains Mono",Menlo,monospace;font-size:9pt;background:#F1EFE8;padding:0 3px;border-radius:3px}
pre{background:#F1EFE8;padding:8pt;border-radius:6px;font-size:8.5pt;white-space:pre-wrap;word-break:break-word}
pre code{background:none;padding:0}
table{border-collapse:collapse;width:100%;font-size:9.5pt;margin:6pt 0;page-break-inside:auto}
th,td{border:1px solid #D9D6CE;padding:3pt 5pt;vertical-align:top;text-align:left}
th{background:#F7F5EE}
tr{page-break-inside:avoid}
blockquote{border-left:3px solid #C63C0A;margin:0;padding:0 10pt;color:#6B6F82}
hr{border:0;border-top:1px solid #D9D6CE;margin:14pt 0}
img{max-width:100%}
</style>"""


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def html_de_markdown(md: Path) -> str:
    try:
        import markdown  # type: ignore
    except ImportError:
        raise SystemExit("INDISPONIVEL: o modulo `markdown` nao esta instalado (pip install markdown); "
                         "o PDF do manual NAO foi refeito")
    corpo = markdown.markdown(md.read_text(encoding="utf-8"), extensions=["tables", "fenced_code"])
    return CSS_MD + corpo


def imprimir(html: Path, pdf: Path, paisagem: bool) -> None:
    script = f"""
import {{ chromium }} from '{PLAYWRIGHT}';
const b = await chromium.launch(); const p = await b.newPage();
await p.goto('file://{html}', {{ waitUntil: 'networkidle' }}); await p.waitForTimeout(600);
await p.emulateMedia({{ media: 'print' }});
await p.pdf({{ path: '{pdf}', format: 'A4', landscape: {str(paisagem).lower()}, printBackground: true, scale: {0.78 if paisagem else 1},
  margin: {{ top: '{'8mm' if paisagem else '14mm'}', bottom: '{'8mm' if paisagem else '14mm'}', left: '{'8mm' if paisagem else '14mm'}', right: '{'8mm' if paisagem else '14mm'}' }} }});
await b.close();
"""
    with tempfile.NamedTemporaryFile("w", suffix=".mjs", delete=False) as f:
        f.write(script)
    r = subprocess.run(["node", f.name], capture_output=True, text=True, timeout=180)
    Path(f.name).unlink(missing_ok=True)
    if r.returncode:
        raise SystemExit(f"falhou ao imprimir {pdf.name}: {r.stderr.strip()[:300]}")


def main() -> int:
    forcar, conferir = "--forcar" in sys.argv, "--conferir" in sys.argv
    estado = json.loads(ESTADO.read_text(encoding="utf-8")) if ESTADO.is_file() else {}
    desatualizados = []
    for fonte, pdf in ALVOS:
        if not fonte.is_file():
            print(f"  fonte ausente: {fonte.relative_to(RAIZ)}")
            desatualizados.append(pdf.name)
            continue
        h = sha(fonte)
        em_dia = pdf.is_file() and estado.get(pdf.name) == h
        if em_dia and not forcar:
            print(f"  ok {pdf.name} em dia com {fonte.name}")
            continue
        if conferir:
            desatualizados.append(pdf.name)
            print(f"  refaria {pdf.name} ({fonte.name} mudou)")
            continue
        if fonte.suffix == ".md":
            tmp = Path(tempfile.mkdtemp()) / "manual.html"
            tmp.write_text(html_de_markdown(fonte), encoding="utf-8")
            imprimir(tmp, pdf, paisagem=False)
        else:
            imprimir(fonte, pdf, paisagem=True)
        estado[pdf.name] = h
        print(f"  ok {pdf.name} refeito de {fonte.name} ({pdf.stat().st_size // 1024} KiB)")
    if not conferir:
        ESTADO.write_text(json.dumps(estado, indent=2) + "\n", encoding="utf-8")
    if desatualizados:
        print(f"PDF desatualizado: {', '.join(desatualizados)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
