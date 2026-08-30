# Update README test counts
# 29/08 00:45

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
alvo = '''O motor de armazenamento está completo e testado: **374 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 211), **587 no projeto inteiro**, sem
nenhuma dependência externa (só a `std`) — o que faz o projeto compilar
offline.'''
novo = '''O motor de armazenamento está completo e testado: **386 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 223), **615 no projeto inteiro**, sem
nenhuma dependência externa (só a `std`) — o que faz o projeto compilar
offline.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
