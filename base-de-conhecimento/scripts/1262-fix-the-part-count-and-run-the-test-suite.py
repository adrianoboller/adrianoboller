# Fix the part count and run the test suite
# 30/08 06:30

p='docs/PENDENCIAS.md'; s=open(p,encoding='utf-8').read()
velho='orquestra as **dezessete** partes'
novo='orquestra as **dezoito** partes'
assert s.count(velho)==1, s.count(velho)
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('dezessete -> dezoito, contado pelo --listar')
