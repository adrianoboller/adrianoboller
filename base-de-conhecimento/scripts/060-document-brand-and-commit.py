# Document brand and commit
# 27/08 19:18

p='/home/user/adrianoboller/CLAUDE.md'
s=open(p).read()
s=s.replace('''## Regras que não se quebram''','''## A marca é oficial

Os arquivos estão em `phxsql/marca/`, com a especificação em
`phxsql/marca/LEIA-ME.md`. Tipografia **Exo 2**, fundo `#010418`, assinatura
*Built to store. Engineered to scale.*

A marca **manda** sobre qualquer paleta inventada. Duas adaptações já
decididas e documentadas: o corpo de texto longo não usa Exo 2, e o vermelhão
escurece para `#C63C0A` no tema claro, por contraste.

Atenção: a folha de marca afirma *ACID compliant* e *built-in replication*, e
**nenhum dos dois é verdade hoje**. Não repita essas afirmações em documento
técnico enquanto não forem.

## Regras que não se quebram''')
open(p,'w').write(s)
