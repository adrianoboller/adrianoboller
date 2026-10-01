#!/usr/bin/env python3
"""Fontes e imagens da marca dentro do HTML, como data URI.

A pagina publicada e UM arquivo: o visualizador de artefatos bloqueia imagem e fonte de
qualquer outra origem, e sem rede o Google Fonts cai calado no fallback (o PhxSql mediu:
`document.fonts.check()` diz «true» para o fallback). Por isso nada vem de fora.

A LISTA das fontes nao se digita: sai das regras @font-face do `app.css` da interface, que e
quem decide quais faces a marca usa. Face nova la entra aqui sozinha; face apagada la sai.
"""
from __future__ import annotations

import base64
import re
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
ASSETS = RAIZ / "apps/phxclaw-ui/assets"
TIPOS = {".png": "image/png", ".woff2": "font/woff2", ".svg": "image/svg+xml"}


def data_uri(caminho: Path) -> str:
    tipo = TIPOS[caminho.suffix]
    return f"data:{tipo};base64," + base64.b64encode(caminho.read_bytes()).decode("ascii")


def faces_do_app_css() -> list[dict]:
    """Cada @font-face do app.css: familia, peso, estilo e o arquivo local."""
    css = (ASSETS / "app.css").read_text(encoding="utf-8")
    faces = []
    for bloco in re.findall(r"@font-face\{([^}]*)\}", css):
        fam = re.search(r'font-family:"([^"]+)"', bloco).group(1)
        peso = re.search(r"font-weight:([^;]+)", bloco).group(1).strip()
        estilo = re.search(r"font-style:([^;]+)", bloco).group(1).strip()
        arq = re.search(r'url\("\./([^"]+)"\)', bloco).group(1)
        faces.append({"familia": fam, "peso": peso, "estilo": estilo, "arquivo": ASSETS / arq})
    if not faces:
        raise SystemExit("PARADA: nenhuma @font-face no app.css -- a marca ficaria sem fonte, calada")
    return faces


def css_das_faces() -> str:
    return "\n".join(
        f'@font-face{{font-family:"{f["familia"]}";font-style:{f["estilo"]};'
        f'font-weight:{f["peso"]};font-display:swap;src:url({data_uri(f["arquivo"])}) format("woff2")}}'
        for f in faces_do_app_css())


def dimensoes(caminho: Path) -> tuple[int, int]:
    """Largura e altura de um PNG, do cabecalho IHDR: o tamanho da imagem nao se digita."""
    import struct
    cab = caminho.read_bytes()[:24]
    if cab[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"PARADA: {caminho.name} nao e PNG")
    return struct.unpack(">II", cab[16:24])
