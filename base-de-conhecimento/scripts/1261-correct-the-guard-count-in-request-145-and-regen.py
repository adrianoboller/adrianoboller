# Correct the guard count in request 145 and regenerate
# 30/08 06:29

import re
p='docs/PENDENCIAS.md'; s=open(p,encoding='utf-8').read()
# A linha do 145 vinha com os numeros da corrida DAQUELA frente. Depois do
# merge ela passou a descrever um catalogo maior -- e numero de documento que
# nao acompanha o merge e numero que envelhece calado.
velho='Medido: 19 guardas, 17 provadas e 2 redundantes'
novo='Medido: 27 guardas no catalogo apos a integracao de tres frentes (19 provadas na ultima corrida completa, 2 redundantes)'
assert s.count(velho)==1, s.count(velho)
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('numero do 145 corrigido para o catalogo medido')
