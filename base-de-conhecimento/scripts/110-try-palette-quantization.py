# Try palette quantization
# 27/08 20:07

from PIL import Image
import os
src = Image.open('marca/derivados/phxsql-simbolo-224.png')
for cores in (256, 192, 128, 96):
    q = src.quantize(colors=cores, method=Image.FASTOCTREE)
    q.save(f'/tmp/q{cores}.png', optimize=True)
    print(cores, os.path.getsize(f'/tmp/q{cores}.png'), 'B')
