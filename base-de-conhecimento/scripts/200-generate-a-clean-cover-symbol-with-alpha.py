# Generate a clean cover symbol with alpha
# 27/08 21:28

from PIL import Image
import os

logo = Image.open('marca/phxsql-logo.png').convert('RGB')
w, h = logo.size
# Só a fênix e o cilindro. A palavra "PhxSql" fica de fora: na capa ela já
# aparece grande, em texto, logo abaixo -- e era o topo dela que estava
# cortado no meio.
logo.crop((int(w*0.105), int(h*0.095), int(w*0.895), int(h*0.565))).save('/tmp/simbolo-capa.png')

def extrair(origem, largura, destino, cores):
    im = Image.open(origem).convert('RGB')
    e = largura / im.width
    im = im.resize((largura, max(1, round(im.height*e))), Image.LANCZOS)
    px = im.load(); s = Image.new('RGBA', im.size); sp = s.load()
    for y in range(im.height):
        for x in range(im.width):
            r,g,b = px[x,y]
            r=max(0,r-1); g=max(0,g-4); b=max(0,b-23)
            a=max(r,g,b)
            if a == 0:
                sp[x,y]=(0,0,0,0)
            else:
                k=255/a
                sp[x,y]=(min(255,round(r*k)),min(255,round(g*k)),min(255,round(b*k)),a)
    s.quantize(colors=cores, method=Image.FASTOCTREE).save(destino, optimize=True)
    return Image.open(destino), os.path.getsize(destino)

im, n = extrair('/tmp/simbolo-capa.png', 440, 'marca/derivados/phxsql-simbolo-440.png', 224)
print(f'simbolo novo: {im.size}, {n} bytes ({round(n*4/3)} em base64)')

# como fica nos dois fundos do dossie
S='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/'
a = Image.open('marca/derivados/phxsql-simbolo-440.png').convert('RGBA')
alvo = a.resize((300, round(300*a.height/a.width)), Image.LANCZOS)
comp = Image.new('RGB', (640, alvo.height + 20), (247,245,242))   # papel claro
comp.paste(alvo, (10,10), alvo)
escuro = Image.new('RGB', (300, alvo.height), (13,17,26))          # tema escuro
escuro.paste(alvo, (0,0), alvo)
comp.paste(escuro, (330,10))
comp.save(S+'conferir-capa.png')
