#!/usr/bin/env python3
"""Converte PDF em Markdown legivel, sem perder o localizador.

Por que nao basta "extrair o texto": o que um agente cita de um PDF precisa
apontar para pagina, e o .md tem de guardar isso. Cada pagina vira uma secao
com marcador `<!-- pagina N -->`, e o cabecalho traz o SHA-256 do PDF de
origem: citacao sem origem conferivel nao vale como evidencia.

O que este script NAO faz, de proposito:
  - nao inventa texto de pagina que nao tem texto (PDF escaneado): a pagina
    fica marcada OCR_REQUERIDO com o tamanho em bytes, e alguem decide;
  - nao "melhora" o conteudo: reflui paragrafo quebrado por coluna, marca
    titulo e bloco de codigo, e para por ai. Interpretacao e do agente, com o
    localizador na mao;
  - nao grava segredo: token e chave privada saem substituidos, e o cabecalho
    diz quantas vezes isso aconteceu.

Uso:
  pdf_para_markdown.py --pdf inputs/estoque-codigo.pdf --saida .wx-migration/extraidos
  pdf_para_markdown.py --pdf a.pdf --saida x/ --linguagem wlanguage --minimo 40
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import date
from pathlib import Path

TOKEN = re.compile(r"ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}")
# Uma linha e titulo quando e curta, nao termina em pontuacao de frase e vem
# isolada. Heuristica: erra para menos, e titulo perdido vira paragrafo, que e
# melhor que paragrafo virado titulo.
RX_TITULO = re.compile(r"^[A-ZÁÉÍÓÚÂÊÔÃÕÇ0-9][^.;:]{2,70}$")
# Linhas que denunciam codigo, por linguagem. Serve para abrir bloco ```; o que
# nao casar fica como texto, que e o padrao seguro.
CODIGO = {
    "wlanguage": re.compile(r"\b(PROCEDURE|FIN|SI\b|ALORS|SINON|POUR TOUT|HLitRecherche|HAjoute|HModifie|HSupprime|HExecuteRequete|SI |RENVOYER|RESULTAT)\b|^\s*//|\bQUAND EXCEPTION\b|::|\.\.[A-Za-z]"),
    "php": re.compile(r"<\?php|\$[A-Za-z_]\w*\s*=|\bfunction\s+\w+\s*\(|->\w+\(|\becho\b|\brequire(_once)?\b"),
    "sql": re.compile(r"\b(SELECT|INSERT INTO|UPDATE|DELETE FROM|CREATE TABLE|ALTER TABLE|JOIN|WHERE|GROUP BY|ORDER BY)\b", re.I),
    "nenhuma": re.compile(r"(?!x)x"),
}


def sha256(caminho: Path) -> str:
    h = hashlib.sha256()
    with caminho.open("rb") as f:
        for pedaco in iter(lambda: f.read(1 << 20), b""):
            h.update(pedaco)
    return h.hexdigest()


def paginas_do_pdf(pdf: Path) -> list[str]:
    """Texto por pagina. pypdf primeiro; pdfminer se nao houver. Sem nenhum dos
    dois, o script diz e sai com 3, sem inventar."""
    try:
        from pypdf import PdfReader  # type: ignore
        return [(p.extract_text() or "") for p in PdfReader(str(pdf)).pages]
    except ImportError:
        pass
    try:
        from pdfminer.high_level import extract_text  # type: ignore
        return (extract_text(str(pdf)) or "").split("\f")
    except ImportError:
        raise SystemExit("erro: instale pypdf (pip install pypdf) ou pdfminer.six; sem um deles nao ha extracao, e este script nao inventa texto")


def refluir(bruto: str) -> list[str]:
    """Junta linha quebrada no meio de frase; preserva linha em branco (que
    separa paragrafo) e linha que parece codigo ou item de lista."""
    linhas = [l.rstrip() for l in bruto.replace("\r\n", "\n").split("\n")]
    saida: list[str] = []
    for l in linhas:
        anterior = saida[-1] if saida else ""
        junta = (anterior and l and not anterior.endswith((".", ":", ";", "?", "!"))
                 and l[:1].islower() and not l.lstrip().startswith(("-", "*", "|", "//"))
                 and len(anterior) > 40)
        if junta:
            saida[-1] = anterior + " " + l.lstrip()
        else:
            saida.append(l)
    return saida


def marcar(linhas: list[str], rx_codigo: re.Pattern) -> list[str]:
    """Titulo vira ###; sequencia de linhas de codigo vira bloco cercado."""
    saida: list[str] = []
    dentro = False
    for i, l in enumerate(linhas):
        eh_codigo = bool(l.strip()) and bool(rx_codigo.search(l))
        if eh_codigo and not dentro:
            saida.append("```"); dentro = True
        elif dentro and not eh_codigo and not l.strip():
            saida.append("```"); dentro = False
        if dentro:
            saida.append(l)
            continue
        seguinte = linhas[i + 1] if i + 1 < len(linhas) else ""
        anterior = linhas[i - 1] if i else ""
        if l.strip() and RX_TITULO.match(l.strip()) and not anterior.strip() and (not seguinte.strip() or seguinte[:1].isupper()):
            saida.append(f"### {l.strip()}")
        else:
            saida.append(l)
    if dentro:
        saida.append("```")
    return saida


def converter(pdf: Path, linguagem: str, minimo: int) -> tuple[str, dict]:
    paginas = paginas_do_pdf(pdf)
    rx = CODIGO.get(linguagem, CODIGO["nenhuma"])
    h = sha256(pdf)
    ocr: list[int] = []
    redigidos = 0
    corpo: list[str] = []
    for n, bruto in enumerate(paginas, 1):
        texto = bruto or ""
        texto, k = TOKEN.subn("<segredo omitido>", texto)
        redigidos += k
        corpo.append(f"<!-- pagina {n} -->")
        corpo.append(f"## Página {n}")
        corpo.append("")
        if len(texto.strip()) < minimo:
            ocr.append(n)
            corpo += [f"> **OCR_REQUERIDO** — esta página tem {len(texto.strip())} caracteres de texto extraível "
                      f"(mínimo {minimo}). Provavelmente é imagem. Nada foi inventado aqui: rode OCR ou peça a página "
                      f"em outro formato antes de citar qualquer coisa dela.", ""]
            continue
        corpo += marcar(refluir(texto), rx) + [""]
    resumo = {"pdf": pdf.name, "sha256": h, "paginas": len(paginas),
              "paginas_ocr_requerido": ocr, "segredos_omitidos": redigidos,
              "caracteres": sum(len((p or "").strip()) for p in paginas)}
    cab = ["---", f"origem: {pdf.name}", f"sha256: {h}", f"paginas: {len(paginas)}",
           f"extraido_em: {date.today().isoformat()}", "extraido_por: pdf_para_markdown.py",
           f"linguagem_de_codigo: {linguagem}", "---", "",
           f"# {pdf.stem}", "",
           f"Convertido de `{pdf.name}` ({len(paginas)} páginas) em {date.today().isoformat()}. "
           "Cada página é uma seção com o marcador `<!-- pagina N -->`: **cite sempre a página**, e confira o `sha256` acima contra o PDF de origem.", ""]
    if ocr:
        cab += [f"> **{len(ocr)} página(s) sem texto extraível** ({', '.join(map(str, ocr[:20]))}"
                + (", …" if len(ocr) > 20 else "") + "). Elas estão marcadas `OCR_REQUERIDO` e **não têm conteúdo aqui**.", ""]
    if redigidos:
        cab += [f"> {redigidos} trecho(s) com formato de token ou chave foram substituídos por `<segredo omitido>`.", ""]
    return "\n".join(cab + corpo).rstrip() + "\n", resumo


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--pdf", required=True, type=Path)
    p.add_argument("--saida", required=True, type=Path, help="pasta onde gravar o .md")
    p.add_argument("--linguagem", default="nenhuma", choices=sorted(CODIGO))
    p.add_argument("--minimo", type=int, default=30, help="caracteres minimos por pagina antes de marcar OCR_REQUERIDO")
    p.add_argument("--project-root", type=Path, default=Path("."))
    p.add_argument("--forcar", action="store_true", help="regrava o .md se ja existir")
    a = p.parse_args()
    if not a.pdf.is_file():
        print(f"erro: {a.pdf} nao existe", file=sys.stderr)
        return 2
    destino = a.saida / (a.pdf.stem + ".md")
    if destino.exists() and not a.forcar:
        print(f"erro: {destino} ja existe; use --forcar para regravar", file=sys.stderr)
        return 2
    md, resumo = converter(a.pdf, a.linguagem, a.minimo)
    a.saida.mkdir(parents=True, exist_ok=True)
    destino.write_text(md, encoding="utf-8")
    (a.saida / (a.pdf.stem + ".json")).write_text(json.dumps(resumo, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    aviso = f", {len(resumo['paginas_ocr_requerido'])} pagina(s) OCR_REQUERIDO" if resumo["paginas_ocr_requerido"] else ""
    seg = f", {resumo['segredos_omitidos']} segredo(s) omitido(s)" if resumo["segredos_omitidos"] else ""
    print(f"MARKDOWN {destino} ({resumo['paginas']} paginas, sha {resumo['sha256'][:12]}{aviso}{seg})")
    return 0


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    raise SystemExit(registro.envolver(__file__, main))
