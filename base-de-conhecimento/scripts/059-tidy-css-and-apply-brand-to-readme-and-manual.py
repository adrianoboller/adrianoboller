# Tidy CSS and apply brand to README and manual
# 27/08 19:17

p='README.md'
s=open(p).read()
s=s.replace('''# PhxSql

Motor de dados em Rust''','''<img src="marca/derivados/phxsql-logo-560.png" alt="PhxSql" width="260">

# PhxSql — Phoenix Database Engine

> Built to store. Engineered to scale.

Motor de dados em Rust''')
s=s.replace('''## Licença''','''## Marca

Os arquivos oficiais estão em [`marca/`](marca/): manual de marca, logotipo,
tela de abertura e os derivados usados na documentação.

| | |
|---|---|
| Tipografia | Exo 2 — SemiBold / Medium / Regular |
| Fundo | `#010418` |
| Paleta | `#FFC43D` `#FF8A1C` `#FF4D10` `#D71A1A` `#8B0D0D` `#DDE2EB` |

## Licença''')
open(p,'w').write(s)

p='MANUAL.txt'
s=open(p).read()
s=s.replace('''================================================================================
PHXSQL - MANUAL DO OPERADOR
Motor de dados no modelo de arquivos separados do HFSQL   |   Linux e Windows
================================================================================''','''================================================================================
PHXSQL - PHOENIX DATABASE ENGINE
Manual do operador                                        |   Linux e Windows
Built to store. Engineered to scale.
================================================================================''')
s=s.replace('''================================================================================
PhxSql - cinco arquivos, uma tabela.
================================================================================''','''================================================================================
PhxSql - cinco arquivos, uma tabela.
Built to store. Engineered to scale.
================================================================================''')
open(p,'w').write(s)
print("README e MANUAL com a marca")
