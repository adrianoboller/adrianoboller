# Quantize derivatives and inspect UI markup
# 27/08 20:08

from PIL import Image
import os
for nome, cores in [('phxsql-simbolo-224.png',192), ('phxsql-icone-64.png',128), ('phxsql-icone-32.png',96)]:
    p=f'marca/derivados/{nome}'
    Image.open(p).quantize(colors=cores, method=Image.FASTOCTREE).save(p, optimize=True)
    print(f'{nome:26} {os.path.getsize(p):>6} B')
