# Extract brand symbol with alpha at UI sizes
# 27/08 20:07

from PIL import Image
import os

def extrair(origem, largura, destino):
    """Tira o fundo #010418 e devolve o simbolo com alfa.

    O logo e brilho sobre fundo quase preto: subtrair o fundo e desfazer a
    pre-multiplicacao recupera a cor real de cada pixel, e a borda do brilho
    sai suave em vez de recortada."""
    im = Image.open(origem).convert('RGB')
    escala = largura / im.width
    im = im.resize((largura, max(1, round(im.height * escala))), Image.LANCZOS)
    px = im.load()
    saida = Image.new('RGBA', im.size)
    sp = saida.load()
    fr, fg, fb = 1, 4, 23
    for y in range(im.height):
        for x in range(im.width):
            r, g, b = px[x, y]
            r = max(0, r - fr); g = max(0, g - fg); b = max(0, b - fb)
            a = max(r, g, b)
            if a == 0:
                sp[x, y] = (0, 0, 0, 0)
            else:
                k = 255 / a
                sp[x, y] = (min(255, round(r*k)), min(255, round(g*k)), min(255, round(b*k)), a)
    saida.save(destino, optimize=True)
    return os.path.getsize(destino)

# recorta o simbolo do logo quadrado (so a fenix + o cilindro, sem a palavra)
logo = Image.open('marca/phxsql-logo.png').convert('RGB')
w, h = logo.size
simbolo = logo.crop((int(w*0.16), int(h*0.11), int(w*0.84), int(h*0.545)))
simbolo.save('/tmp/simbolo-cru.png')
print('recorte:', simbolo.size)

for larg, nome in [(224,'phxsql-simbolo-224.png'), (64,'phxsql-icone-64.png'), (32,'phxsql-icone-32.png')]:
    n = extrair('/tmp/simbolo-cru.png', larg, f'marca/derivados/{nome}')
    print(f'{nome:28} {n:>7} bytes  ->  base64 {round(n*4/3):>7}')
