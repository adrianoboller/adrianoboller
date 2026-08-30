# Document the brand fix and do a final render check
# 27/08 21:30

p='marca/LEIA-ME.md'
s=open(p).read()
s=s.replace('''| `phxsql-simbolo-420.png` | capa do dossiê (embutido como data URI) |''',
            '''| `phxsql-simbolo-420.png` | aposentado — trazia a palavra cortada ao meio |
| `phxsql-simbolo-440.png` | capa do dossiê (embutido como data URI) |''')
s=s.replace('''O ícone da barra usa um **recorte mais fechado** que o do cartão''',
'''O símbolo de 440 substituiu o de 420 na capa do dossiê por dois defeitos que
apareceram juntos: o recorte antigo pegava o **topo da palavra "PhxSql"**, que
saía cortada ao meio, e a imagem **não tinha alfa**, então o `#010418` virava
um retângulo escuro solto sobre o papel claro do documento.

O recorte novo pega só a fênix e o cilindro — a palavra já aparece grande,
em texto, logo abaixo. E o alfa resolveu metade do problema, não todo: sobre
papel claro o **cilindro vira um fantasma branco**, porque o miolo escuro dele
era o fundo aparecendo. Por isso a capa põe a marca numa **placa** com o
`#010418`, com cantos arredondados e um brilho — deliberada, para ler como
apresentação da marca e não como retângulo perdido. Assim a mesma imagem
serve aos dois temas do dossiê.

O ícone da barra usa um **recorte mais fechado** que o do cartão''')
open(p,'w').write(s)
