# Fix the cover numbers and validate
# 28/08 17:09

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
for a,b in [
 ('<div><div class="v">32.812</div>','<div><div class="v">34.156</div>'),
 ('<div><div class="v">440</div>','<div><div class="v">453</div>'),
 ('<div><div class="v">5.465</div>','<div><div class="v">5.619</div>'),
]:
    assert a in s, a; s=s.replace(a,b,1)
open(p,'w').write(s)
