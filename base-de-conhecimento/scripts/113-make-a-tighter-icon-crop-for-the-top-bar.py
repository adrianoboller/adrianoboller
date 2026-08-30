# Make a tighter icon crop for the top bar
# 27/08 20:09

from PIL import Image
import os
logo = Image.open('marca/phxsql-logo.png').convert('RGB')
w,h = logo.size
# recorte fechado: so a ave e o cilindro. As pontas da asa e as trilhas de
# circuito somem em 30 px -- em miniatura, menos desenho e mais legivel.
logo.crop((int(w*0.255), int(h*0.125), int(w*0.745), int(h*0.545))).save('/tmp/icone-cru.png')

def extrair(origem, largura, destino, cores):
    im = Image.open(origem).convert('RGB')
    e = largura/im.width
    im = im.resize((largura, max(1,round(im.height*e))), Image.LANCZOS)
    px = im.load(); s = Image.new('RGBA', im.size); sp = s.load()
    for y in range(im.height):
        for x in range(im.width):
            r,g,b = px[x,y]
            r=max(0,r-1); g=max(0,g-4); b=max(0,b-23)
            a=max(r,g,b)
            if a==0: sp[x,y]=(0,0,0,0)
            else:
                k=255/a
                sp[x,y]=(min(255,round(r*k)),min(255,round(g*k)),min(255,round(b*k)),a)
    s.quantize(colors=cores, method=Image.FASTOCTREE).save(destino, optimize=True)
    return Image.open(destino).size, os.path.getsize(destino)

for larg,nome,c in [(72,'phxsql-icone-64.png',128),(40,'phxsql-icone-32.png',96)]:
    print(nome, *extrair('/tmp/icone-cru.png', larg, f'marca/derivados/{nome}', c))

im=Image.open('marca/derivados/phxsql-icone-64.png').convert('RGBA')
alvo=im.resize((30, round(30*im.height/im.width)), Image.LANCZOS)
f=Image.new('RGB',(300,52),(10,17,34))
f.paste(alvo,(16,(52-alvo.height)//2),alvo)
f.resize((900,156), Image.NEAREST).save('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/conferir-barra.png')
