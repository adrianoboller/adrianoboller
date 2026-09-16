# Normaliza fotos de produto para o canvas de estudio da Bambu Lab (#F7F7F7).
#
#     python3 normaliza-canvas.py entrada.jpg saida.jpg [tolerancia]
#
# Pinta so o fundo, por flood-fill a partir das quatro bordas e com tolerancia
# pequena (6 niveis), para nao tocar o produto nem os sombreados; se a foto nao
# for 1:1, encaixa num quadrado da mesma cor. Existe porque a galeria da loja
# passou a ser um canvas unico: foto em fundo branco puro sobre canvas cinza
# desenha um retangulo — foi o defeito visto na foto da A1 Combo em 16/09/2026.
import sys
from PIL import Image
from collections import deque
CANVAS = (247, 247, 247)

def normaliza(src, dst, tol=6):
    im = Image.open(src).convert('RGB'); w, h = im.size; px = im.load()
    bg = px[0, 0]
    seen = bytearray(w * h); q = deque()
    for x in range(w): q.append((x, 0)); q.append((x, h - 1))
    for y in range(h): q.append((0, y)); q.append((w - 1, y))
    n = 0
    while q:
        x, y = q.popleft(); i = y * w + x
        if seen[i]: continue
        seen[i] = 1; r, g, b = px[x, y]
        if abs(r - bg[0]) > tol or abs(g - bg[1]) > tol or abs(b - bg[2]) > tol: continue
        px[x, y] = CANVAS; n += 1
        if x > 0: q.append((x - 1, y))
        if x < w - 1: q.append((x + 1, y))
        if y > 0: q.append((x, y - 1))
        if y < h - 1: q.append((x, y + 1))
    if w != h:
        s = max(w, h); sq = Image.new('RGB', (s, s), CANVAS); sq.paste(im, ((s - w) // 2, (s - h) // 2)); im = sq
    im.save(dst, quality=92)
    return bg, round(100 * n / (w * h), 1), im.size

if __name__ == '__main__':
    a, b = sys.argv[1], sys.argv[2]; tol = int(sys.argv[3]) if len(sys.argv) > 3 else 6
    bg, pct, size = normaliza(a, b, tol)
    print(f'{a}: fundo medido {bg}, {pct}% pintado -> {b} {size[0]}x{size[1]}')
