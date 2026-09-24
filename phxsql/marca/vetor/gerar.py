#!/usr/bin/env python3
"""Gera os SVG derivados da marca a partir do simbolo desenhado a mao.

    <venv com fonttools>/bin/python marca/vetor/gerar.py <Exo2[wght].ttf>

O `phx-simbolo.svg` e a unica fonte desenhada a mao. Tudo o mais sai daqui:
a versao monocromatica, o icone pequeno, e as palavras (PhxSql, PhxZip,
PhxMail, Phxblockchain) em CURVAS -- o SVG da palavra nao depende de a Exo 2
estar instalada em quem abre.

`fontTools` e ferramenta de TRABALHO, como o p7zip do teste do PhxZip: roda
aqui para gerar os arquivos versionados, e o produto nao a conhece. A pétrea
de zero dependencias vale para o binario, e o binario nao le isto.
"""
import re
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

AQUI = Path(__file__).resolve().parent
PRATA = "#DDE2EB"
# Acento de cada produto -- so cores da paleta oficial (marca/LEIA-ME.md).
FAMILIA = {
    "PhxSql": "#FF8A1C",
    "PhxZip": "#FFC43D",
    "PhxMail": "#FF4D10",
    "Phxblockchain": "#D71A1A",
}


def fonte(caminho, peso=600):
    f = TTFont(caminho)
    return instantiateVariableFont(f, {"wght": peso}, inplace=False)


def palavra(f, texto, altura, cores):
    """Caminhos SVG da palavra: devolve (lista de (d, cor), largura)."""
    upm = f["head"].unitsPerEm
    cap = f["OS/2"].sCapHeight or int(upm * 0.7)
    esc = altura / cap
    cmap = f.getBestCmap()
    gs = f.getGlyphSet()
    hmtx = f["hmtx"]
    x = 0.0
    saida = []
    for i, ch in enumerate(texto):
        nome = cmap[ord(ch)]
        pen = SVGPathPen(gs)
        # y cresce para baixo no SVG: espelha e sobe a linha de base.
        tp = TransformPen(pen, (esc, 0, 0, -esc, x, altura))
        gs[nome].draw(tp)
        saida.append((pen.getCommands(), cores(i, ch)))
        x += hmtx[nome][0] * esc
    return saida, x


def fmt(d):
    return re.sub(r"(\d+\.\d{2})\d+", r"\1", d)


# O simbolo de cada produto. Todos sao a mesma fenix; o PhxZip a pousa na
# morsa (pedido do dono, 24/09/2026 -- `gerar-phxzip.py`).
SIMBOLO = {"PhxZip": "phxzip-simbolo-morsa.svg"}


def simbolo_interno(produto=None):
    s = (AQUI / SIMBOLO.get(produto, "phx-simbolo.svg")).read_text(encoding="utf-8")
    corpo = s[s.index(">", s.index("<svg")) + 1 : s.rindex("</svg>")]
    corpo = re.sub(r"<title>.*?</title>", "", corpo, flags=re.S)
    return corpo


def horizontal(f, produto, acento, assinatura=True):
    """Simbolo a esquerda, palavra a direita, o `x` na cor do produto."""
    alt = 150
    caminhos, larg = palavra(f, produto, alt, lambda i, ch: acento if i == 2 else PRATA)
    x0 = 560
    L = int(x0 + larg + 40)
    H = 512
    y0 = 170
    partes = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {L} {H}" role="img" aria-label="{produto}">',
        f"<title>{produto} — logotipo horizontal</title>",
        f'<svg x="0" y="0" width="512" height="512" viewBox="0 0 512 512">{simbolo_interno(produto)}</svg>',
        f'<line x1="536" y1="120" x2="536" y2="400" stroke="{PRATA}" stroke-opacity=".25" stroke-width="3"/>',
        f'<g transform="translate({x0} {y0})">',
    ]
    for d, cor in caminhos:
        partes.append(f'<path fill="{cor}" d="{fmt(d)}"/>')
    partes.append("</g>")
    # o traço de luz sob a palavra, como na folha de marca
    partes.append(
        f'<defs><linearGradient id="luz" x1="0" x2="1"><stop offset="0" stop-color="{acento}" stop-opacity="0"/>'
        f'<stop offset=".5" stop-color="{acento}"/><stop offset="1" stop-color="{acento}" stop-opacity="0"/></linearGradient></defs>'
    )
    # abaixo das descendentes (q, p): na altura da linha de base o traco
    # cortava as letras -- visto na primeira captura
    partes.append(f'<rect x="{x0}" y="{y0 + alt + 56}" width="{larg:.0f}" height="4" fill="url(#luz)"/>')
    if assinatura:
        ass, lg = palavra(f, "BUILT TO STORE. ENGINEERED TO SCALE.", 22, lambda i, ch: PRATA)
        k = larg / lg if lg > larg else 1
        partes.append(f'<g transform="translate({x0} {y0 + alt + 84}) scale({k:.4f})" opacity=".75">')
        for d, cor in ass:
            partes.append(f'<path fill="{cor}" d="{fmt(d)}"/>')
        partes.append("</g>")
    partes.append("</svg>\n")
    return "\n".join(partes)


def monocromatico():
    """Uma cor so (`currentColor`): impressao, carimbo, gravacao a laser."""
    s = (AQUI / "phx-simbolo.svg").read_text(encoding="utf-8")
    s = s.replace("Phoenix — símbolo (fênix e cilindro)", "Phoenix — símbolo monocromático")
    s = re.sub(r'<circle cx="256" cy="300" r="150"[^>]*/>', "", s)
    # O cilindro precisa TAPAR as asas atras dele, senao as duas formas se
    # fundem numa mancha (visto na primeira captura). O tapa-buraco e a cor
    # do papel: branco por padrao, e `--vazio` troca para fundo escuro.
    s = re.sub(r'fill="url\(#corpo\)"', 'style="fill:var(--vazio,#fff)"', s)
    s = re.sub(r'fill="#0a1122"', 'style="fill:var(--vazio,#fff)"', s)
    s = re.sub(r'fill="(url\(#[a-z]+\)|#[0-9A-Fa-f]{6})"', 'fill="currentColor"', s)
    s = re.sub(r'stroke="(url\(#[a-z]+\)|#[0-9A-Fa-f]{6})"', 'stroke="currentColor"', s)
    s = re.sub(r'fill="currentColor"( opacity="[.\d]+")', r'fill="currentColor"', s)
    # o olho e o furo do cilindro precisam de VAZIO, nao de cor
    s = s.replace('<circle cx="314" cy="134" r="4" fill="currentColor"/>', "")
    return s


def icone():
    """Para 16 a 48 px: sem trilhas de circuito, sem as luzes do cilindro --
    em 30 px o desenho completo vira borrao (a lição do icone-64 da barra)."""
    s = (AQUI / "phx-simbolo.svg").read_text(encoding="utf-8")
    s = s.replace("Phoenix — símbolo (fênix e cilindro)", "Phoenix — ícone (tamanhos pequenos)")
    s = re.sub(r'\s*<use href="#trilhas-esq"[^>]*/>', "", s)
    s = re.sub(r'<g fill="#FFC43D">.*?</g>', "", s, flags=re.S)
    s = re.sub(r'<circle cx="256" cy="300" r="150"[^>]*/>', "", s)
    # Traco mais grosso e cilindro mais claro: em 32 px o traco de 5 some e
    # o corpo escuro vira borrao sobre o fundo escuro (visto na captura).
    s = s.replace('stroke-width="5"', 'stroke-width="10"').replace('stroke-width="4"', 'stroke-width="8"')
    s = s.replace('stop-color="#1a2440"', 'stop-color="#34487a"')
    s = s.replace('<g fill="none" stroke="#FF8A1C" stroke-width="8">', '<g fill="none" stroke="#FFC43D" stroke-width="8">')
    s = s.replace('viewBox="0 0 512 512"', 'viewBox="40 40 432 432"')
    return s


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    f = fonte(sys.argv[1])
    (AQUI / "phx-simbolo-mono.svg").write_text(monocromatico(), encoding="utf-8")
    (AQUI / "phx-icone.svg").write_text(icone(), encoding="utf-8")
    for produto, acento in FAMILIA.items():
        nome = produto.lower() + "-horizontal.svg"
        (AQUI / nome).write_text(horizontal(f, produto, acento), encoding="utf-8")
        print("gerado", nome)
    print("gerados phx-simbolo-mono.svg e phx-icone.svg")
    return 0


if __name__ == "__main__":
    sys.exit(main())
