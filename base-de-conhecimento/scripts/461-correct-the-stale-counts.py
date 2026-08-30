# Correct the stale counts
# 28/08 15:16

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
pares=[
 ('<p><strong>36 das 39 operações do protocolo têm tela.</strong> Fora:',
  '<p><strong>46 das 49 operações do protocolo têm tela.</strong> Fora:'),
 ('Ele troca o nome exibido de qualquer um dos 82 rótulos da barra e dos',
  'Ele troca o nome exibido de qualquer um dos 88 rótulos da barra e dos'),
 ('View Database · grade de tabelas e ficha de edição · 33 das 36 ops na tela',
  'View Database · grade de tabelas e ficha de edição · 46 das 49 ops na tela'),
 ('Barra de ferramentas · 21 ferramentas, 17 vivas e 4 dizendo o que falta',
  'Barra de ferramentas · 22 ferramentas, 18 vivas e 4 dizendo o que falta'),
 ('Barra de menu tradicional · nove menus · Alt, setas e Esc',
  'Barra de menu tradicional · nove menus, 57 itens · Alt, setas e Esc'),
]
for a,b in pares:
    assert a in s, a
    s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
