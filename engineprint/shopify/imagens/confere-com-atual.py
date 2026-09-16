import json,os,subprocess,math
from PIL import Image, ImageDraw
ATUAL={
 'A1mini':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/9daff4905f8c9a0f411dd607fda7e94f-5e8ee4370cc25f2a6717571711978643-1024-1024.webp',
 'A1':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/impressora_3d_bambu_lab_a1_combo_com_ams_original_2_20260506154836_56416e9b9d8d.webp',
 'P1S':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/NWBMB00005_1.webp',
 'P2S':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/p2s-combo-6a4d3a5d8dc31.webp',
 'X2D':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/cdc125cb13454b8494491698cb96cc00-w69kbjw94k.webp',
 'H2S':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/bambu-lab-h2s-combo-1-1ov6ddkzz3.webp',
 'H2C':'https://cdn.shopify.com/s/files/1/0689/5498/0395/files/h2c-1-wj0rw4l9cw.webp'}
os.makedirs('atual',exist_ok=True)
for m,u in ATUAL.items():
    d=f'atual/{m}.webp'
    if not os.path.exists(d): subprocess.run(['curl','-s','-o',d,u],check=False)
sel=json.load(open('escolhidas.json'))
cat=json.load(open('catalogo.json'))
CELL=290; HEAD=24
def sheet(mods,nome):
    cols=1+max(len(sel[m]) for m in mods)
    s=Image.new('RGB',(cols*CELL,len(mods)*(CELL+HEAD)),(255,255,255)); d=ImageDraw.Draw(s)
    for r,m in enumerate(mods):
        y=r*(CELL+HEAD)
        cells=[('atual/%s.webp'%m,'ATUAL '+m)]
        for u in sel[m]:
            i=cat[m].index(u); cells.append((f'img/{m}/{i:02d}.jpg','NOVA %02d'%i))
        for c,(f,lab) in enumerate(cells):
            x=c*CELL
            try:
                im=Image.open(f).convert('RGB'); im.thumbnail((CELL-6,CELL-6))
                s.paste(im,(x+(CELL-im.width)//2,y+HEAD+(CELL-HEAD-im.height)//2))
            except Exception as e: print('erro',f,e)
            col=(20,110,200) if c==0 else (200,20,20)
            d.rectangle([x+2,y+2,x+118,y+HEAD-2],fill=col)
            d.text((x+8,y+7),lab,fill=(255,255,255))
            d.line([x,y,x,y+CELL+HEAD],fill=(205,205,205))
        d.line([0,y,cols*CELL,y],fill=(120,120,120))
    s.save(nome); print(nome,s.size)
sheet(['A1mini','A1','P1S','P2S'],'confere-1.png')
sheet(['X2D','H2S','H2C'],'confere-2.png')
