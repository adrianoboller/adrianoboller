#!/bin/sh
# Video das telas na sequencia de uso: um quadro de 1920x1080 por tela, 4 s cada.
# Ferramenta de FORA do produto (ffmpeg do pacote Python imageio-ffmpeg + Pillow);
# o binario do PhxSql nao depende de nada disto.
#
# Reconstruido em 01/10/2026 do comando que gerou PhxSql-telas-claro.mp4
# (5 min 16 s, h264, 7.726.326 bytes). A ordem sai do roteiro.json -- a corrida
# original lia uma copia (ordem.json), conferida igual: 79 ids na mesma ordem.
#
# Uso: sh video.sh <pasta-com-os-svg-do-capturar-telas.mjs> <saida.mp4>
set -e
SVG=$1; SAIDA=$2
AQUI=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d)
python3 - "$AQUI/../roteiro.json" "$SVG" "$TMP" <<'E'
import re, base64, glob, os, json, io, sys
from PIL import Image
roteiro, svg, tmp = sys.argv[1:]
ordem = [t["id"] for c in json.load(open(roteiro))["capitulos"] for t in c["telas"]]
por = {int(os.path.basename(f).split('-')[0]): f for f in glob.glob(f'{svg}/*-claro.svg')}
falta = [i for i in ordem if i not in por]
if falta:
    raise SystemExit(f'telas sem captura: {falta}')
with open(f'{tmp}/lista.txt', 'w') as lista:
    for k, i in enumerate(ordem):
        s = open(por[i]).read()
        im = Image.open(io.BytesIO(base64.b64decode(
            re.search(r'base64,([A-Za-z0-9+/=]+)', s).group(1)))).convert('RGB')
        W, H, mw, mh = 1920, 1080, 1856, 1016
        r = min(mw / im.width, mh / im.height)
        im = im.resize((round(im.width * r), round(im.height * r)), Image.LANCZOS)
        q = Image.new('RGB', (W, H), (244, 246, 250))
        q.paste(im, ((W - im.width) // 2, (H - im.height) // 2))
        q.save(f'{tmp}/{k + 1:03d}.png')
        lista.write(f"file '{tmp}/{k + 1:03d}.png'\nduration 4\n")
    lista.write(f"file '{tmp}/{len(ordem):03d}.png'\n")
print(len(ordem), 'quadros')
E
F=$(python3 -c "import imageio_ffmpeg;print(imageio_ffmpeg.get_ffmpeg_exe())")
"$F" -y -loglevel error -f concat -safe 0 -i "$TMP/lista.txt" -vf "fps=25,format=yuv420p" \
  -c:v libx264 -preset medium -crf 20 -movflags +faststart "$SAIDA"
rm -rf "$TMP"
