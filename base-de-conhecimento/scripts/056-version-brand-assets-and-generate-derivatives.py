# Version brand assets and generate derivatives
# 27/08 19:15

from PIL import Image
from collections import Counter

base="/home/user/adrianoboller/phxsql/marca"

# --- cor de fundo real da marca ---
logo=Image.open(f"{base}/phxsql-logo.png").convert("RGB")
cantos=[logo.getpixel(p) for p in [(4,4),(logo.width-5,4),(4,logo.height-5),(logo.width-5,logo.height-5)]]
fundo=Counter(cantos).most_common(1)[0][0]
print(f"fundo da marca: #{fundo[0]:02X}{fundo[1]:02X}{fundo[2]:02X}  (amostrado dos 4 cantos)")

folha=Image.open(f"{base}/phxsql-manual-de-marca.png").convert("RGB")
print(f"fundo da folha: #%02X%02X%02X" % folha.getpixel((8,8)))

# --- derivados para uso na pagina ---
for nome, largura in [("logo", 560), ("icone", 128)]:
    im = logo.copy()
    im.thumbnail((largura, largura), Image.LANCZOS)
    saida=f"{base}/derivados/phxsql-{nome}-{largura}.png"
    im.save(saida, optimize=True)
    import os
    print(f"  {os.path.basename(saida):<28} {im.width}x{im.height}  {os.path.getsize(saida)/1024:.0f} KB")

# marca so o simbolo (recorta a parte de cima da logo quadrada, sem o texto)
simbolo = logo.crop((150, 150, 1104, 780))
simbolo.thumbnail((420, 420), Image.LANCZOS)
simbolo.save(f"{base}/derivados/phxsql-simbolo-420.png", optimize=True)
import os
print(f"  phxsql-simbolo-420.png       {simbolo.width}x{simbolo.height}  {os.path.getsize(f'{base}/derivados/phxsql-simbolo-420.png')/1024:.0f} KB")
