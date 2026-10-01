#!/usr/bin/env python3
"""Prova real dos entregaveis do phxclaw-office com leitores independentes.

Uso:
  validar.py docx|xlsx|pptx ARQUIVO MODELO.json   confere o arquivo contra o modelo
  validar.py gerar PASTA                         gera py.docx e py.xlsx (deflate)

Sai com codigo 0 se tudo confere; imprime o motivo e sai com 1 se nao.
O modelo e o mesmo JSON que o agente manda ao Rust, entao a expectativa nao
e digitada duas vezes. A regra dos caracteres proibidos do XML 1.0 e
reimplementada aqui de proposito: e a segunda opiniao sobre o que o Rust tira.
"""

import json
import posixpath
import re
import sys
import zipfile
import xml.etree.ElementTree as ET

PROIBIDOS = re.compile("[^\u0009\u000a\u000d -퟿-�\U00010000-\U0010ffff]")


def limpo(s):
    return PROIBIDOS.sub("", s).replace("\r", "")


def falha(msg):
    print("FALHA: " + msg)
    sys.exit(1)


def igual(visto, esperado, onde):
    if visto != esperado:
        falha(f"{onde}: visto {visto!r}, esperado {esperado!r}")


def conferir_pacote(caminho):
    """Toda parte e XML bem formado, toda parte tem tipo de conteudo e todo
    alvo de relacao interna existe. E o que o Office confere antes de abrir."""
    with zipfile.ZipFile(caminho) as z:
        nomes = set(z.namelist())
        if z.testzip() is not None:
            falha("crc invalido no zip")
        ct = ET.fromstring(z.read("[Content_Types].xml"))
        ns = "{http://schemas.openxmlformats.org/package/2006/content-types}"
        defaults = {d.get("Extension").lower() for d in ct.iter(ns + "Default")}
        overrides = {o.get("PartName").lstrip("/") for o in ct.iter(ns + "Override")}
        for o in overrides:
            if o not in nomes:
                falha(f"override para parte inexistente: {o}")
        for n in nomes:
            dados = z.read(n)
            try:
                ET.fromstring(dados)
            except ET.ParseError as e:
                falha(f"{n} nao e XML bem formado: {e}")
            if n != "[Content_Types].xml" and n not in overrides:
                ext = n.rsplit(".", 1)[-1].lower()
                if ext not in defaults:
                    falha(f"{n} sem tipo de conteudo")
            if n.endswith(".rels"):
                base = posixpath.dirname(posixpath.dirname(n))
                for r in ET.fromstring(dados):
                    if r.get("TargetMode") == "External":
                        continue
                    t = r.get("Target")
                    alvo = t.lstrip("/") if t.startswith("/") else posixpath.normpath(posixpath.join(base, t))
                    if alvo not in nomes:
                        falha(f"{n}: relacao {r.get('Id')} aponta para {alvo}, que nao existe")


def validar_docx(caminho, modelo):
    import docx

    d = docx.Document(caminho)
    igual(d.core_properties.title, limpo(modelo["title"]), "titulo nas propriedades")
    paras = [p for p in d.paragraphs]
    while paras and paras[-1].text == "" and not paras[-1].runs:
        paras.pop()
    esperados = []
    if modelo["title"]:
        esperados.append(("Title", limpo(modelo["title"]), False, False))
    tabelas = []
    for b in modelo["blocks"]:
        t = b["type"]
        if t == "heading":
            esperados.append((f"Heading {b['level']}", limpo(b["text"]), False, False))
        elif t == "paragraph":
            esperados.append(("Normal", limpo(b["text"]), b.get("bold", False), False))
        elif t == "bullets":
            for it in b["items"]:
                esperados.append(("List Paragraph", limpo(it), False, True))
        elif t == "table":
            tabelas.append(b)
    igual(len(paras), len(esperados), "numero de paragrafos")
    for i, (p, (estilo, texto, negrito, marcador)) in enumerate(zip(paras, esperados)):
        igual(p.style.name, estilo, f"estilo do paragrafo {i}")
        igual(p.text, texto, f"texto do paragrafo {i}")
        if negrito:
            if not all(r.bold for r in p.runs):
                falha(f"paragrafo {i} deveria estar em negrito")
        tem_num = p._p.pPr is not None and p._p.pPr.numPr is not None
        igual(tem_num, marcador, f"marcador do paragrafo {i}")
    igual(len(d.tables), len(tabelas), "numero de tabelas")
    for ti, (tab, b) in enumerate(zip(d.tables, tabelas)):
        linhas = ([b["header"]] if b["header"] else []) + b["rows"]
        ncol = max(len(r) for r in linhas)
        esperado = [[limpo(c) for c in r] + [""] * (ncol - len(r)) for r in linhas]
        visto = [[c.text for c in row.cells] for row in tab.rows]
        igual(visto, esperado, f"tabela {ti}")
        if b["header"] and not all(r.bold for c in tab.rows[0].cells for p in c.paragraphs for r in p.runs):
            falha(f"cabecalho da tabela {ti} deveria estar em negrito")


def validar_xlsx(caminho, modelo):
    import openpyxl

    wb = openpyxl.load_workbook(caminho)
    igual(wb.sheetnames, [s["name"] for s in modelo["sheets"]], "nomes das abas")
    for s in modelo["sheets"]:
        ws = wb[s["name"]]
        for r, linha in enumerate(s.get("rows", []), start=1):
            for c, cel in enumerate(linha, start=1):
                x = ws.cell(row=r, column=c)
                onde = f"{s['name']}!{x.coordinate}"
                if cel == "empty":
                    igual(x.value, None, onde)
                    continue
                (tipo, v), = cel.items()
                if tipo == "text":
                    igual(x.data_type, "s", onde + " tipo")
                    igual(x.value, limpo(v), onde)
                elif tipo == "number":
                    # O ponto da prova: numero chega como numero, nao texto.
                    igual(x.data_type, "n", onde + " tipo")
                    igual(float(x.value), float(v), onde)
                elif tipo == "bool":
                    igual(x.data_type, "b", onde + " tipo")
                    igual(x.value, v, onde)
                elif tipo == "formula":
                    igual(x.data_type, "f", onde + " tipo")
                    igual(x.value, "=" + v.lstrip("="), onde)
                if s.get("bold_header") and r == 1:
                    igual(bool(x.font.b), True, onde + " negrito")
                elif r == 1:
                    igual(bool(x.font.b), False, onde + " negrito")


def validar_pptx(caminho, modelo):
    import pptx

    p = pptx.Presentation(caminho)
    slides = list(p.slides)
    igual(len(slides), 1 + len(modelo["slides"]), "numero de slides")
    igual(slides[0].shapes.title.text, limpo(modelo["title"]), "titulo da capa")
    igual(slides[0].slide_layout.name, "Title Slide", "layout da capa")
    for i, (sl, m) in enumerate(zip(slides[1:], modelo["slides"]), start=2):
        igual(sl.shapes.title.text, limpo(m["title"]), f"titulo do slide {i}")
        corpo = [ph for ph in sl.placeholders if ph.placeholder_format.idx == 1]
        igual(len(corpo), 1, f"corpo do slide {i}")
        itens = [pa.text.replace("\v", "\n") for pa in corpo[0].text_frame.paragraphs]
        esperado = [limpo(b) for b in m.get("bullets", [])] or [""]
        igual(itens, esperado, f"marcadores do slide {i}")
        if m.get("notes"):
            if not sl.has_notes_slide:
                falha(f"slide {i} sem anotacoes")
            igual(sl.notes_slide.notes_text_frame.text, limpo(m["notes"]), f"anotacoes do slide {i}")
        else:
            igual(sl.has_notes_slide, False, f"slide {i} com anotacao inesperada")


def gerar(pasta):
    import docx
    import openpyxl

    d = docx.Document()
    d.add_heading("Título gerado & lido", level=1)
    d.add_paragraph("Parágrafo com ação\tapós tab")
    t = d.add_table(rows=1, cols=2)
    t.rows[0].cells[0].text = "São Paulo"
    t.rows[0].cells[1].text = "SP"
    d.save(f"{pasta}/py.docx")

    wb = openpyxl.Workbook()
    ws = wb.active
    ws.title = "Plan ção"
    ws["A1"] = "maçã"
    ws["B1"] = 3.25
    ws["C3"] = True
    ws["D3"] = "=B1*2"
    wb.create_sheet("Segunda")["B2"] = 7
    wb.save(f"{pasta}/py.xlsx")

    import pptx

    p = pptx.Presentation()
    s = p.slides.add_slide(p.slide_layouts[1])
    s.shapes.title.text = "Agenda & ação"
    s.placeholders[1].text_frame.text = "primeiro ponto"
    s.notes_slide.notes_text_frame.text = "lembrar do prazo"
    s2 = p.slides.add_slide(p.slide_layouts[5])
    s2.shapes.title.text = "Fim"
    p.save(f"{pasta}/py.pptx")


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "gerar":
        gerar(sys.argv[2])
        print("ok")
        return
    if len(sys.argv) != 4:
        falha(__doc__)
    tipo, caminho, modelo = sys.argv[1], sys.argv[2], json.load(open(sys.argv[3], encoding="utf-8"))
    conferir_pacote(caminho)
    {"docx": validar_docx, "xlsx": validar_xlsx, "pptx": validar_pptx}[tipo](caminho, modelo)
    print("ok")


if __name__ == "__main__":
    main()
