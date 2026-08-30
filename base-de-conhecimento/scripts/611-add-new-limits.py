# Add new limits
# 28/08 18:03

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()
velho='''| Precisão de `Decimal` | 38 dígitos |'''
novo='''| Precisão de `Decimal` | 38 dígitos |
| Texto do motivo no `.reason` | 2 000 bytes |
| Identidade no `.reason` | 512 bytes |
| Colunas externas numa linha do `.trash` | 255 |
| Tamanho de um registro do `.trash` | 4 GiB |'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
