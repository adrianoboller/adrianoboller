#!/usr/bin/env python3
"""Confere midia onde ela APARECE, nao no contador.

`mediaCount` e `status: READY` dizem que o Shopify processou o arquivo, nao que
a imagem serve para a pagina. Em 17/09/2026 a H2D tinha seis midias READY e a
capa era de 389 px com fundo PRETO dentro de um palco #F7F7F7 — o defeito que o
dono ja tinha apontado, escondido atras de um numero que anunciava sucesso.

Este script responde as tres perguntas que a API nao responde:

  1. a dimensao aguenta o palco? (o palco pede >= 2x o tamanho em CSS)
  2. o canto bate com o fundo do palco? (um canto fora ja denuncia)
  3. as irmas combinam entre si? — desenha todas no palco de verdade, porque o
     que estraga um carrossel e a DIFERENCA entre as imagens, e essa nenhuma
     delas carrega sozinha.

Uso:
    python3 confere-no-palco.py folha.png <arquivo-ou-url> [...]
"""
import io
import sys
import urllib.request

from PIL import Image, ImageDraw

PALCO = (0xF7, 0xF7, 0xF7)  # .pp__stage { background: var(--pp-canvas, #f7f7f7) }
CSS = 440                   # largura do palco no celular, medida
LADO = 250                  # lado de cada quadro na folha
BORDA = 12
TOLERANCIA = 12             # diferenca de canal que ainda conta como "bate"


def abre(fonte):
    if fonte.startswith(("http://", "https://")):
        with urllib.request.urlopen(fonte) as r:
            return Image.open(io.BytesIO(r.read())).convert("RGB")
    return Image.open(fonte).convert("RGB")


def canto(im, lado=10):
    """Cor media dos quatro cantos: diz o fundo real, nao o declarado.

    O resize para 1x1 com BOX e a media exata do quadrado, e evita varrer
    pixel a pixel."""
    w, h = im.size
    pontos = [(0, 0), (w - lado, 0), (0, h - lado), (w - lado, h - lado)]
    medias = [
        im.crop((x, y, x + lado, y + lado)).resize((1, 1), Image.BOX).getpixel((0, 0))
        for x, y in pontos
    ]
    return tuple(sum(m[i] for m in medias) // len(medias) for i in range(3))


def no_palco(im, lado=LADO):
    """Desenha como o tema desenha: object-fit contain sobre o palco."""
    w, h = im.size
    e = min(lado / w, lado / h)
    menor = im.resize((max(1, round(w * e)), max(1, round(h * e))), Image.LANCZOS)
    cv = Image.new("RGB", (lado, lado), PALCO)
    cv.paste(menor, ((lado - menor.width) // 2, (lado - menor.height) // 2))
    return cv


def main(saida, fontes):
    colunas = min(6, len(fontes))
    linhas = (len(fontes) + colunas - 1) // colunas
    folha = Image.new(
        "RGB",
        (BORDA + colunas * (LADO + BORDA), BORDA + linhas * (LADO + 60)),
        (255, 255, 255),
    )
    d = ImageDraw.Draw(folha)
    reprovadas = 0

    for i, fonte in enumerate(fontes):
        im = abre(fonte)
        w, h = im.size
        fundo = canto(im)
        # 2x porque a tela do celular tem densidade 2; abaixo disso borra
        pequena = min(w, h) < CSS * 2
        destoa = max(abs(fundo[c] - PALCO[c]) for c in range(3)) > TOLERANCIA
        if pequena or destoa:
            reprovadas += 1

        x = BORDA + (i % colunas) * (LADO + BORDA)
        y = BORDA + (i // colunas) * (LADO + 60)
        folha.paste(no_palco(im), (x, y))
        d.rectangle([x, y, x + LADO - 1, y + LADO - 1], outline=(190, 190, 190))
        nome = fonte.split("/")[-1].split("?")[0][:28]
        d.text((x + 4, y + LADO + 6), f"{nome}  {w}x{h}",
               fill=(170, 20, 20) if pequena else (10, 10, 10))
        d.text((x + 4, y + LADO + 24), f"canto {fundo}",
               fill=(170, 20, 20) if destoa else (20, 120, 20))

        aviso = []
        if pequena:
            aviso.append(f"PEQUENA (palco pede {CSS * 2}px)")
        if destoa:
            aviso.append(f"FUNDO {fundo} != {PALCO}")
        print(f"{nome:30s} {w}x{h:<6} canto {fundo}  {' | '.join(aviso) or 'ok'}")

    folha.save(saida)
    print(f"\n{len(fontes)} conferidas, {reprovadas} reprovadas -> {saida}")
    print("Olhe a folha: o que reprova sozinha o laco acha; o que so aparece")
    print("na diferenca entre as irmas so aparece olhando.")
    return 1 if reprovadas else 0


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1], sys.argv[2:]))
