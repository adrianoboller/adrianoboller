#!/usr/bin/env python3
"""Gera as imagens de identidade da tela do PhxClaw a partir das artes do dono.

Uso: python3 scripts/marca_da_tela.py DIR_DAS_ARTES
  DIR_DAS_ARTES contem icone.png (app, quadrado arredondado), fenix.png (fenix limpa),
  fenix-palavra.png (fenix com arco + «Phoenix»). Cada arte e PNG quadrado/4:3 sobre o
  fundo escuro da marca.

Por que um script e nao recorte a mao: a arte muda, e recorte feito a mao ninguem refaz
igual. O fundo escuro embutido vira transparencia por CHAVE de cor (distancia ao fundo),
porque no tema claro a marca cai sobre papel #f7f5f2 e um quadrado azul-noite ali seria
uma mancha, nao uma marca.
"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw

RAIZ = Path(__file__).resolve().parent.parent
ASSETS = RAIZ / "apps/phxclaw-ui/assets"
TAURI = RAIZ / "apps/phxclaw-desktop/src-tauri/icons/icon.png"
FUNDO = (1, 4, 24)  # token --fundo, #010418
TINTA_CLARO = (1, 4, 24)  # a palavra branca vira a cor do fundo da marca sobre o papel


def chave(im, limiar0=10, limiar1=80):
    """Fundo escuro -> transparente. alpha cresce com a distancia ao fundo amostrado."""
    im = im.convert("RGB")
    w, h = im.size
    amostras = [im.getpixel(p) for p in [(4, 4), (w - 5, 4), (4, h - 5), (w - 5, h - 5)]]
    bg = tuple(sum(c[i] for c in amostras) // len(amostras) for i in range(3))
    out = Image.new("RGBA", im.size)
    px, po = im.load(), out.load()
    for y in range(h):
        for x in range(w):
            c = px[x, y]
            d = max(c[0] - bg[0], c[1] - bg[1], c[2] - bg[2], 0)
            a = min(max((d - limiar0) / (limiar1 - limiar0), 0.0), 1.0)
            if a <= 0:
                po[x, y] = (0, 0, 0, 0)
                continue
            # tira o fundo misturado da borda (desmultiplica), senao fica halo azul no papel
            cor = tuple(min(max(int(bg[i] + (c[i] - bg[i]) / a), 0), 255) for i in range(3))
            po[x, y] = cor + (int(a * 255),)
    return out


def aparar(im, folga=0.04):
    caixa = im.getchannel("A").point(lambda v: 255 if v > 12 else 0).getbbox()
    im = im.crop(caixa)
    m = int(max(im.size) * folga)
    tela = Image.new("RGBA", (im.width + 2 * m, im.height + 2 * m), (0, 0, 0, 0))
    tela.paste(im, (m, m))
    return tela


def quadrar(im):
    lado = max(im.size)
    tela = Image.new("RGBA", (lado, lado), (0, 0, 0, 0))
    tela.paste(im, ((lado - im.width) // 2, (lado - im.height) // 2))
    return tela


def tinta_no_claro(im):
    """Variante do tema claro: o que e branco/cinza (a palavra, o brilho) vira tinta escura;
    o laranja da fenix fica como esta."""
    im = im.copy()
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            sat = max(r, g, b) - min(r, g, b)
            if sat < 60 and max(r, g, b) > 120:
                px[x, y] = TINTA_CLARO + (a,)
    return im


def salvar(im, caminho, cores=256):
    # 256 cores com pontilhado: o degrade do quadrado arredondado nao vira faixa, e o 512
    # cabe abaixo dos ~150 KB que a casca do celular baixa na primeira visita.
    caminho.parent.mkdir(parents=True, exist_ok=True)
    if cores and im.mode == "RGBA":
        im = im.quantize(colors=cores, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.FLOYDSTEINBERG)
    elif cores:
        im = im.quantize(colors=cores, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG)
    im.save(caminho, optimize=True)
    print(f"{caminho.relative_to(RAIZ)}  {Image.open(caminho).size}  {caminho.stat().st_size} B")


def main(dir_artes):
    d = Path(dir_artes)
    icone = Image.open(d / "icone.png").convert("RGB")
    fenix = aparar(chave(Image.open(d / "fenix.png")))
    palavra = aparar(chave(Image.open(d / "fenix-palavra.png")), folga=0.02)

    # Icone do aplicativo: o quadrado arredondado da arte, recortado com pouca folga, sobre
    # o --fundo (iOS e Android pintam de preto o que for transparente num icone opaco).
    cx, cy, meio = 627, 620, 520
    qd = icone.crop((cx - meio, cy - meio, cx + meio, cy + meio))
    for lado in (192, 512):
        salvar(qd.resize((lado, lado), Image.LANCZOS), ASSETS / f"icone-{lado}.png", cores=192)

    # Mascaravel: o lancador recorta em circulo/gota; a zona segura e o circulo de 80%.
    # A fenix limpa a 62% do lado, sobre o --fundo inteiro, nao perde ponta de asa.
    m = Image.new("RGBA", (512, 512), FUNDO + (255,))
    f = fenix.copy()
    f.thumbnail((int(512 * 0.62),) * 2, Image.LANCZOS)
    m.alpha_composite(f, ((512 - f.width) // 2, (512 - f.height) // 2))
    salvar(m.convert("RGB"), ASSETS / "icone-mascaravel-512.png")

    # Desktop (Tauri): o mesmo quadrado arredondado, com os cantos transparentes.
    t = qd.resize((256, 256), Image.LANCZOS).convert("RGBA")
    mascara = Image.new("L", (256 * 4, 256 * 4), 0)
    ImageDraw.Draw(mascara).rounded_rectangle((18, 18, 256 * 4 - 19, 256 * 4 - 19), radius=165, fill=255)
    t.putalpha(mascara.resize((256, 256), Image.LANCZOS))
    # O tauri::generate_context! recusa PNG de paleta («icon.png is not RGBA»): sem quantizar.
    salvar(t, TAURI, cores=0)

    # Barra superior e favicon: fenix limpa, transparente (vale nos dois temas).
    fq = quadrar(fenix)
    for lado, nome in ((96, "marca-96.png"), (32, "favicon-32.png"), (48, "favicon-48.png")):
        salvar(fq.resize((lado, lado), Image.LANCZOS), ASSETS / nome)

    # Abertura: fenix com arco e a palavra. Duas variantes: escuro (palavra branca) e
    # claro (palavra na tinta do fundo da marca).
    p = palavra.copy()
    # 1,5x do tamanho na tela (251 px): no celular com rede lenta cada KB da abertura
    # disputa banda com o CSS e o JS do caminho critico (M4/M5 da qualificacao).
    p.thumbnail((390, 390), Image.LANCZOS)
    salvar(p, ASSETS / "abertura-escuro.png", cores=128)
    salvar(tinta_no_claro(p), ASSETS / "abertura-claro.png", cores=128)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
