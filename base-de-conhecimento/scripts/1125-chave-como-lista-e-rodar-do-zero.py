# Chave como lista e rodar do zero
# 29/08 11:43

import io
p='bancada/dblink/prova-sincronia.py'
s=io.open(p,encoding='utf-8').read()
s=s.replace('"chave": {"id": 1}','"chave": [1]').replace('"chave": {"id": 2}','"chave": [2]')
io.open(p,'w',encoding='utf-8').write(s)
