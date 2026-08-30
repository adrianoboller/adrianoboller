# Shift the last two figure numbers
# 28/08 15:11

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
# figuras 18 e 19 viram 20 e 21 -- as duas novas secoes trazem a 18 e a 19
s=s.replace('<b>Figura 19.</b>','<b>Figura 21.</b>',1)
s=s.replace('<b>Figura 18.</b>','<b>Figura 20.</b>',1)
open(p,'w').write(s)
