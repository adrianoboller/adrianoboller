# Update brand readme and dossier measurement
# 27/08 20:11

p='marca/LEIA-ME.md'
s=open(p).read()
velho = '''| `phxsql-logo-560.png` | cabeçalho do `README.md` |
| `phxsql-simbolo-420.png` | capa do dossiê (embutido como data URI) |
| `phxsql-icone-128.png` | uso geral em miniatura |'''
novo = '''| `phxsql-logo-560.png` | cabeçalho do `README.md` |
| `phxsql-simbolo-420.png` | capa do dossiê (embutido como data URI) |
| `phxsql-icone-128.png` | uso geral em miniatura |
| `phxsql-simbolo-224.png` | cartão de entrada do Centro de Controle |
| `phxsql-icone-64.png` | barra do Centro de Controle |
| `phxsql-icone-32.png` | ícone da aba do navegador |

Os três últimos são os que entram na interface web, embutidos no
`ui/index.html` como data URI — a página é um arquivo só, e não há de onde
buscar imagem.

**Eles têm alfa; os originais não.** O fundo `#010418` foi retirado
desfazendo a pré-multiplicação: subtrai o fundo, tira `alfa = max(r,g,b)` e
divide a cor por ele. Como o logo é brilho sobre quase preto, isso recupera
a cor real de cada pixel e a borda do brilho sai suave, em vez de recortada
com halo. É o que permite a fênix assentar em cima do painel `#0a1122` sem
deixar emenda de retângulo.

O ícone da barra usa um **recorte mais fechado** que o do cartão — só a ave e
o cilindro, sem as pontas da asa nem as trilhas de circuito. Em 30 px o
desenho completo vira borrão; menos desenho é mais legível.'''
assert s.count(velho)==1
s = s.replace(velho, novo)
open(p,'w').write(s)
