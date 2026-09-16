from PIL import Image, ImageDraw
import json,os,math
cat=json.load(open('catalogo.json'))
CELL=300; PAD=26
for mod,urls in cat.items():
    files=[f'img/{mod}/{i:02d}.jpg' for i in range(len(urls))]
    files=[f for f in files if os.path.exists(f) and os.path.getsize(f)>2000]
    cols=4; rows=math.ceil(len(files)/cols)
    sheet=Image.new('RGB',(cols*CELL, rows*(CELL+PAD)),(255,255,255))
    d=ImageDraw.Draw(sheet)
    for n,f in enumerate(files):
        try: im=Image.open(f).convert('RGB')
        except Exception as e: print('erro',f,e); continue
        im.thumbnail((CELL-8,CELL-8))
        x=(n%cols)*CELL; y=(n//cols)*(CELL+PAD)
        sheet.paste(im,(x+(CELL-im.width)//2, y+PAD+(CELL-PAD-im.height)//2))
        idx=os.path.basename(f)[:2]
        d.rectangle([x+2,y+2,x+58,y+PAD-2],fill=(200,20,20))
        d.text((x+18,y+7),idx,fill=(255,255,255))
        d.line([x,y,x,y+CELL+PAD],fill=(210,210,210))
    sheet.save(f'folha-{mod}.png')
    print(f'folha-{mod}.png', sheet.size, len(files),'imgs')
