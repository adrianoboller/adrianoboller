# Finish updating, commit and push
# 29/08 02:01

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
s = s.replace("| Master, com a imagem no diário | 18.773 linhas/s |", "| Master, com a imagem no diário | 28.914 linhas/s |")
s = s.replace("| Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |", "| Aplicação, por réplica (as três em paralelo) | 4.357 eventos/s |")
s = s.replace("(`phxsql-core` 163 + `phxsql-store` 223), **615 no projeto inteiro**",
              "(`phxsql-core` 163 + `phxsql-store` 227), **619 no projeto inteiro**")
s = s.replace("O motor de armazenamento está completo e testado: **386 testes** só nele",
              "O motor de armazenamento está completo e testado: **390 testes** só nele")
p.write_text(s)
