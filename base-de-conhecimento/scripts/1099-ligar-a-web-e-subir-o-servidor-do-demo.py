# Ligar a web e subir o servidor do demo
# 29/08 10:20

import json,io
p='$S/demo/config.json'
c=json.load(io.open(p))
c['web']={'ligado':True,'bind':'127.0.0.1:5199'}
json.dump(c, io.open(p,'w'), ensure_ascii=False, indent=1)
print('web ligada em 5199')
