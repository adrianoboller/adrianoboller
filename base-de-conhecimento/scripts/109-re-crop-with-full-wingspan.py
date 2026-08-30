# Re-crop with full wingspan
# 27/08 20:07

from PIL import Image
import os
logo = Image.open('marca/phxsql-logo.png').convert('RGB')
w, h = logo.size
logo.crop((int(w*0.105), int(h*0.095), int(w*0.895), int(h*0.565))).save('/tmp/simbolo-cru.png')

def extrair(origem, largura, destino):
    im = Image.open(origem).convert('RGB')
    e = largura / im.width
    im = im.resize((largura, max(1, round(im.height*e))), Image.LANCZOS)
    px = im.load()
    s = Image.new('RGBA', im.size); sp = s.load()
    for y in range(im.height):
        for x in range(im.width):
            r,g,b = px[x,y]
            r=max(0,r-1); g=max(0,g-4); b=max(0,b-23)
            a=max(r,g,b)
            if a==0: sp[x,y]=(0,0,0,0)
            else:
                k=255/a
                sp[x,y]=(min(255,round(r*k)),min(255,round(g*k)),min(255,round(b*k)),a)
    s.save(destino, optimize=True)
    return s, os.path.getsize(destino)

for larg,nome in [(224,'phxsql-simbolo-224.png'),(64,'phxsql-icone-64.png'),(32,'phxsql-icone-32.png')]:
    im,n = extrair('/tmp/simbolo-cru.png', larg, f'marca/derivados/{nome}')
    print(f'{nome:26} {im.size}  {n:>6} B')

im=Image.open('marca/derivados/phxsql-simbolo-224.png')
f=Image.new('RGB', im.size, (10,17,34)); f.paste(im,(0,0),im)
f.save('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/conferir-simbolo.png')
i2=Image.open('marca/derivados/phxsql-icone-64.png')
f2=Image.new('RGB',(240,90),(10,17,34)); f2.paste(i2.resize((36,int(36*i2.height/i2.width)),Image.LANCZOS),(10,26),i2.resize((36,int(36*i2.height/i2.width)),Image.LANCZOS))
f2.save('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/conferir-icone.png')
