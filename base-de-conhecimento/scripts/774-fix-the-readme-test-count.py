# Fix the README test count
# 28/08 20:04

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
antigo = """O motor de armazenamento está completo e testado: **375 testes**, sem nenhuma
dependência externa (só a `std`), o que faz o projeto compilar offline."""
novo = """O motor de armazenamento está completo e testado: **363 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 200), **567 no projeto inteiro**, sem
nenhuma dependência externa (só a `std`) — o que faz o projeto compilar
offline."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
